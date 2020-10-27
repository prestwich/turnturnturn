use clap::Clap;
use bitcoins::prelude::*;

#[derive(Clap)]
#[clap(version = "1.0", author = "James Prestwich <james@summa.one>")]
pub struct Opts {
    #[clap(short, long)]
    pub message: Option<String>,
    #[clap(short, long)]
    pub fee: Option<u64>,
    #[clap(short, long)]
    pub change_address: Option<String>,
}

impl Opts {
    /// Validate the options. Return a human readable error string
    pub fn validate(&self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(a) = &self.change_address {
            a.parse::<Address>()?;
        }

        if self.fee.is_some() {
            let f = self.fee.unwrap();
            if f > 50_000_000 {
                return Err("Unreasonably high fee".into())
            }
        }

        if let Some(message) = &self.message {
            if message.as_bytes().len() < 12 {
                return Err("Message too short. Must be 12 bytes or more".into());
            }
            if message.as_bytes().len() > 74 {
                return Err("Message too long. Must be 74 bytes or fewer".into());
            }
        }

        Ok(())
    }
}
