use anyhow::Result;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::{
	io::Write,
	{env, fs, sync::Arc},
};

use crate::{
	db::{Db, DbEntry, DynFieldKind},
	encryption::{decrypt_vault, encrypt_vault, password_hash},
};

#[derive(Debug, Deserialize, Serialize)]
struct ConfigFile {
	pub general: ConfigGeneral,
	pub db: ConfigFileDb,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ConfigFileDb {
	pub encrypted: bool,
	salt: String,
	cypher: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct ConfigFileCypher {
	pub contents: Vec<DbEntry>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config {
	#[serde(with = "arc_rwlock_serde")]
	pub general: Arc<RwLock<ConfigGeneral>>,
	#[serde(skip_serializing, with = "arc_rwlock_serde")]
	pub db: Arc<RwLock<Db>>,
	#[serde(rename(serialize = "db"), with = "arc_rwlock_serde")]
	pub config_db: Arc<RwLock<ConfigFileDb>>,
	#[serde(skip_serializing)]
	pub vault_unlocked: bool,
	#[serde(skip_serializing)]
	config_path: String,
	#[serde(skip_serializing)]
	hash: [u8; 32],
}

mod arc_rwlock_serde {
	use parking_lot::RwLock;
	use serde::de::Deserializer;
	use serde::ser::Serializer;
	use serde::{Deserialize, Serialize};
	use std::sync::Arc;

	pub fn serialize<S, T>(val: &Arc<RwLock<T>>, s: S) -> Result<S::Ok, S::Error>
	where
		S: Serializer,
		T: Serialize,
	{
		T::serialize(&*val.read(), s)
	}

	pub fn deserialize<'de, D, T>(d: D) -> Result<Arc<RwLock<T>>, D::Error>
	where
		D: Deserializer<'de>,
		T: Deserialize<'de>,
	{
		Ok(Arc::new(RwLock::new(T::deserialize(d)?)))
	}
}

pub type PresetFields = Vec<(usize, String, String, DynFieldKind)>;

#[derive(Debug, Deserialize, Serialize)]
pub struct ConfigGeneral {
	pub db_timeout: f64,
	pub preset_fields: PresetFields,
}

impl Default for Config {
	fn default() -> Self {
		Config {
			general: Arc::new(RwLock::new(ConfigGeneral {
				db_timeout: 900.0,
				preset_fields: vec![
					(
						0,
						String::from("Custom"),
						String::from(""),
						DynFieldKind::SecretLine,
					),
					(
						1,
						String::from("Username"),
						String::from("Username"),
						DynFieldKind::SecretLine,
					),
					(
						2,
						String::from("Password"),
						String::from("Password"),
						DynFieldKind::SecretLine,
					),
					(3, String::from("Website"), String::from("URL"), DynFieldKind::Url),
					(
						4,
						String::from("Notes"),
						String::from("Notes"),
						DynFieldKind::TextLine,
					),
				],
			})),
			db: Arc::new(RwLock::new(Db::default())),
			vault_unlocked: false,
			config_db: Arc::new(RwLock::new(ConfigFileDb {
				cypher: String::from(""),
				salt: String::from(""),
				encrypted: false,
			})),
			hash: *b"00000000000000000000000000000000",
			config_path: String::from(""),
		}
	}
}

impl From<ConfigFile> for Config {
	fn from(config_file: ConfigFile) -> Self {
		Config {
			general: Arc::new(RwLock::new(ConfigGeneral {
				db_timeout: config_file.general.db_timeout,
				preset_fields: config_file.general.preset_fields,
			})),
			vault_unlocked: false,
			db: Arc::new(RwLock::new(Db::default())),
			config_db: Arc::new(RwLock::new(ConfigFileDb {
				cypher: config_file.db.cypher.clone(),
				encrypted: config_file.db.encrypted,
				salt: config_file.db.salt,
			})),
			hash: *b"00000000000000000000000000000000",
			config_path: String::from(""),
		}
	}
}

impl Config {
	pub fn new() -> Self {
		let cwd = match env::current_dir() {
			Ok(path) => format!("{}", path.display()), // default to current working dir
			Err(_) => String::from(""),                // fallback to root dir
		};

		let path = format!("{}/vault_config.toml", cwd);

		match fs::read_to_string(&path) {
			Ok(content) => {
				let file_contents: ConfigFile = toml::from_str(&content).unwrap();
				let mut config: Config = file_contents.into();
				config.config_path = path.clone();
				config
			}
			Err(_) => {
				println!("writing new config");
				// TODO: start onboarding flow (new password)
				let config = Config {
					config_path: path.clone(),
					..Default::default()
				};
				match fs::write(&path, toml::to_string_pretty(&config).unwrap()) {
					Ok(_) => config,
					Err(_) => panic!("Can't write config file"),
				}
			}
		}
	}

	pub fn decrypt_database(&mut self, password: String) -> Result<()> {
		self.hash = password_hash(password, self.config_db.read().salt.clone())?;
		let contents = if self.config_db.read().encrypted {
			let decrypted =
				decrypt_vault(self.config_db.read().cypher.clone(), self.hash)?;
			self.vault_unlocked = true;
			toml::from_str::<ConfigFileCypher>(decrypted.as_str())?
		} else {
			self.vault_unlocked = true;
			toml::from_str::<ConfigFileCypher>(&self.config_db.read().cypher.clone())?
		};
		self.db.write().contents = contents.contents;
		Ok(())
	}

	fn serialize_db(&self) -> Result<()> {
		// self.db -> self.config_db.cypher as toml
		#[derive(Debug, Serialize, Deserialize)]
		struct DbStruct {
			contents: Vec<DbEntry>,
		}
		let db = DbStruct {
			contents: self.db.read().contents.clone(),
		};
		let mut cypher = toml::to_string(&db)?;
		if self.config_db.read().encrypted {
			cypher = encrypt_vault(cypher, self.hash)?;
		}
		self.config_db.write().cypher = cypher;
		Ok(())
	}

	pub fn save(&self) -> Result<()> {
		self.serialize_db()?;
		let config = toml::to_string_pretty(self)?;
		let mut config_file = fs::OpenOptions::new()
			.write(true)
			.truncate(true)
			.open(self.config_path.clone())?;
		config_file.write_all(config.as_bytes())?;
		config_file.flush()?;
		Ok(())
	}

	pub fn clear_hash(&mut self) {
		// TODO: Eventually zeroize here?
		self.hash = *b"00000000000000000000000000000000";
	}

	pub fn get_field_presets(&self) -> PresetFields {
		self.general.read().preset_fields.clone()
	}
}
