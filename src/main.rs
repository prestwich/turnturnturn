use bitcoins::prelude::*;
use bitcoins_provider::prelude::*;
use coins_bip32::{curve::SigSerialize, HasPrivkey, HasPubkey, Privkey, Pubkey, SigningKey};
use lazy_static::lazy_static;
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{Read, Write},
    time::Duration,
};

use clap::Clap;

mod opts;

#[cfg(all(not(feature = "testnet"), feature = "mainnet"))]
lazy_static! {
    static ref DONATION_ADDRESS: Address = "37hjdPWy9aE4iNbtGRVSpyixXCAZpfePcd".parse().unwrap();
}

#[cfg(all(not(feature = "mainnet"), feature = "testnet"))]
lazy_static! {
    static ref DONATION_ADDRESS: Address = "tb1qm5tfegjevj27yvvna9elym9lnzcf0zraxgl8z2".parse().unwrap();
}

lazy_static! {
    static ref PROVIDER: CachingProvider<EsploraProvider> = Default::default();
}

mod key_ser {
    use super::*;
    use serde::{Deserialize, Deserializer, Serializer};

    /// Strips the '0x' prefix off of hex string so it can be deserialized.
    ///
    /// # Arguments
    ///
    /// * `s` - The hex str
    pub fn strip_0x_prefix(s: &str) -> &str {
        if &s[..2] == "0x" {
            &s[2..]
        } else {
            s
        }
    }

    /// Deserializes a hex string into a u8 array.
    ///
    /// # Arguments
    ///
    /// * `s` - The hex string
    pub fn deserialize_hex(s: &str) -> Result<Vec<u8>, hex::FromHexError> {
        hex::decode(&strip_0x_prefix(s))
    }

    /// Serializes a u8 array into a hex string.
    ///
    /// # Arguments
    ///
    /// * `buf` - The value as a u8 array
    pub fn serialize_hex(buf: &[u8]) -> String {
        format!("0x{}", hex::encode(buf))
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Privkey, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s: &str = Deserialize::deserialize(deserializer)?;
        let buf = deserialize_hex(s).map_err(|e| serde::de::Error::custom(e.to_string()))?;
        let mut k = [0u8; 32];
        k.copy_from_slice(&buf[..32]);
        Ok(Privkey::from_bytes(k).unwrap())
    }

    pub fn serialize<S>(d: &Privkey, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let s: &str = &serialize_hex(d.privkey_bytes().as_ref());
        serializer.serialize_str(s)
    }
}

#[derive(Serialize, Deserialize, Debug)]
struct State {
    #[serde(with = "key_ser")]
    key: Privkey,
    message: String,
    fee: u64,
    change_address: Address,
}

impl State {
    fn pubkey(&self) -> Pubkey {
        self.key.derive_verifying_key().unwrap()
    }

    fn spk(&self) -> ScriptPubkey {
        ScriptPubkey::p2wpkh(&self.pubkey())
    }

    fn address(&self) -> Address {
        Net::encode_address(&self.spk()).unwrap()
    }
}

fn new_ephemeral_key() -> Privkey {
    Privkey::from_bytes(rand::thread_rng().gen()).unwrap()
}

fn read_in_progress() -> Option<State> {
    if let Ok(mut file) = fs::File::open("./inProgress.json") {
        let mut data = String::new();
        file.read_to_string(&mut data).unwrap();
        Some(serde_json::from_str(&data).unwrap())
    } else {
        None
    }
}

fn clear_in_progress(new_name: &str) {
    fs::DirBuilder::new().recursive(true).create("./completed").expect("folder ok");
    let target = format!("./completed/{}.json", new_name);
    fs::rename("./inProgress.json", &target).expect("mv ok");
}

fn write_in_progress(state: &State) {
    let mut file = fs::File::create("./inProgress.json").unwrap();
    let payload = serde_json::to_string(state).unwrap();
    file.write_all(payload.as_bytes()).unwrap();
}

fn build_transaction(
    utxo: &UTXO,
    change_address: Option<Address>,
    message: &str,
    fee: u64,
) -> <Net as Network>::Builder {
    let mut builder = Net::tx_builder()
        .version(2)
        .spend(utxo.outpoint, u32::MAX - 1)
        .op_return(message.as_bytes());

    // ignore small utxos
    let change = utxo.value - fee;
    let address = change_address.unwrap_or_else(|| DONATION_ADDRESS.clone());
    if change > fee * 2 && change > 5_000 {
        builder = builder.pay(change, &address);
    }

    builder
}

fn get_signed_tx(
    utxo: &UTXO,
    change_address: Option<Address>,
    fee: u64,
    state: &State,
) -> BitcoinTx {
    let builder = build_transaction(utxo, change_address, &state.message, fee);

    let tx = builder
        .clone()
        .build_witness()
        .unwrap();
    let sighash_args = utxo.witness_sighash_args(0, Sighash::All).unwrap();
    let sighash = tx.sighash(&sighash_args).unwrap();
    let mut signature = vec![];
    signature.extend(state.key.sign_digest(sighash.into()).unwrap().to_der());
    signature.push(Sighash::All as u8);

    let mut witness: Witness = Vec::new();
    witness.push(signature.into());
    witness.push(state.pubkey().pubkey_bytes().as_ref().into());

    builder
        .extend_witnesses(std::iter::once(witness))
        .build()
        .unwrap()
}

async fn new(options: opts::Opts) -> Result<(), Box<dyn std::error::Error>> {
    let change_address = if let Some(addr) = options.change_address {
        Net::string_to_address(&addr)?
    } else {
        DONATION_ADDRESS.clone()
    };

    let message = match options.message {
        Some(m) => m,
        None => return Err(r#"Must provide a message. Use -m "message text""#.into()),
    };

    let state = State {
        key: new_ephemeral_key(),
        message,
        fee: options.fee.unwrap_or(5000),
        change_address,
    };
    write_in_progress(&state);
    println!("Please send AT LEAST {:?} satoshi to {}", state.fee, state.address());
    resume(&state).await?;
    Ok(())
}

async fn resume(state: &State) -> Result<(), Box<dyn std::error::Error>> {
    process(state).await?;
    Ok(())
}

async fn process(state: &State) -> Result<(), Box<dyn std::error::Error>> {
    let utxos = loop {
        let utxos = PROVIDER.get_utxos_by_script(&state.spk()).await?;
        if !utxos.is_empty() {
            break utxos;
        }
        tokio::time::delay_for(Duration::from_millis(60 * 1000)).await; // wait 1 minute
    };

    let tx = get_signed_tx(&utxos[0], None, state.fee, &state);
    println!("TX blob is\n{:?}", tx.serialize_hex());
    println!("\n\nBroadcasting tx: {:?}", tx.txid().reversed().serialize_hex());
    PROVIDER.broadcast(tx).await.unwrap();

    Ok(())
}

async fn logic() -> Result<(), Box<dyn std::error::Error>> {
    let options = opts::Opts::parse();
    options.validate()?;

    // restore state, or make a new one
    if let Some(s) = read_in_progress() {
        println!("Resuming. Any input was ignored.");
        resume(&s).await?;
        // runs only if above doesn't error (i.e. it worked)
        clear_in_progress(s.address().as_ref());
    } else {
        new(options).await?;
    };

    Ok(())
}

#[tokio::main]
async fn main() {
    println!();
    match logic().await {
        Ok(()) => {},
        Err(e) => println!("{}", e),
    }
}
