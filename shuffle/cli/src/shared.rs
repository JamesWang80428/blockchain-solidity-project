// Copyright (c) The Diem Core Contributors
// SPDX-License-Identifier: Apache-2.0

use crate::deploy;
use anyhow::Result;
use diem_crypto::ed25519::{Ed25519PrivateKey, Ed25519PublicKey};
use diem_sdk::client::BlockingClient;
use diem_types::transaction::authenticator::AuthenticationKey;
use directories::BaseDirs;
use serde::{Deserialize, Serialize};
use serde_generate as serdegen;
use serde_generate::SourceInstaller;
use serde_reflection::Registry;
use std::{
    fs,
    fs::File,
    io,
    io::Write,
    path::{Path, PathBuf},
    time::Duration,
};
use transaction_builder_generator as buildgen;
use transaction_builder_generator::SourceInstaller as BuildgenSourceInstaller;

pub const MAIN_PKG_PATH: &str = "main";

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub struct Config {
    pub(crate) blockchain: String,
}

pub fn get_home_dir() -> PathBuf {
    BaseDirs::new().unwrap().home_dir().to_path_buf()
}

pub fn read_config(project_path: &Path) -> Result<Config> {
    let config_string = fs::read_to_string(project_path.join("Shuffle").with_extension("toml"))?;
    let read_config: Config = toml::from_str(config_string.as_str())?;
    Ok(read_config)
}

/// Send a transaction to the blockchain through the blocking client.
pub fn send(client: &BlockingClient, tx: diem_types::transaction::SignedTransaction) -> Result<()> {
    use diem_json_rpc_types::views::VMStatusView;

    client.submit(&tx)?;
    assert_eq!(
        client
            .wait_for_signed_transaction(&tx, Some(std::time::Duration::from_secs(60)), None)?
            .into_inner()
            .vm_status,
        VMStatusView::Executed,
    );
    Ok(())
}

/// Checks the current directory, and then parent directories for a Shuffle.toml
/// file to indicate the base project directory.
pub fn get_shuffle_project_path(cwd: &Path) -> Result<PathBuf> {
    let mut path: PathBuf = PathBuf::from(cwd);
    let project_file = Path::new("Shuffle.toml");

    loop {
        path.push(project_file);

        if path.is_file() {
            path.pop();
            return Ok(path);
        }

        if !(path.pop() && path.pop()) {
            return Err(anyhow::anyhow!(
                "unable to find Shuffle.toml; are you in a Shuffle project?"
            ));
        }
    }
}

// returns ~/.shuffle
pub fn get_shuffle_dir() -> PathBuf {
    BaseDirs::new().unwrap().home_dir().join(".shuffle")
}

// Contains all the commonly used paths in shuffle/cli
pub struct Home {
    pub shuffle_path: PathBuf,
    pub root_key_path: PathBuf,
    pub node_config_path: PathBuf,
    pub account_dir_path: PathBuf,
    pub latest_dir_path: PathBuf,
    pub latest_key_path: PathBuf,
    pub latest_address_path: PathBuf,
}

impl Home {
    pub fn new(home_dir: &Path) -> Result<Self> {
        Ok(Self {
            shuffle_path: home_dir.join(".shuffle"),
            root_key_path: home_dir.join(".shuffle/nodeconfig/mint.key"),
            node_config_path: home_dir.join(".shuffle/nodeconfig/0/node.yaml"),
            account_dir_path: home_dir.join(".shuffle/accounts"),
            latest_dir_path: home_dir.join(".shuffle/accounts/latest"),
            latest_key_path: home_dir.join(".shuffle/accounts/latest/dev.key"),
            latest_address_path: home_dir.join(".shuffle/accounts/latest/address"),
        })
    }

    pub fn create_archive_dir(&mut self, time: Duration) -> Result<PathBuf> {
        let archived_dir = self.account_dir_path.join(time.as_secs().to_string());
        fs::create_dir(&archived_dir)?;
        Ok(archived_dir)
    }

    pub fn archive_old_key(&mut self, archived_dir: &Path) -> Result<()> {
        let old_key_path = self.latest_key_path.as_path();
        let archived_key_path = archived_dir.join("dev.key");
        fs::copy(old_key_path, archived_key_path)?;
        Ok(())
    }

    pub fn archive_old_address(&mut self, archived_dir: &Path) -> Result<()> {
        let old_address_path = self.latest_address_path.as_path();
        let archived_address_path = archived_dir.join("address");
        fs::copy(old_address_path, archived_address_path)?;
        Ok(())
    }

    pub fn generate_shuffle_accounts_dir(&mut self) -> Result<()> {
        if !self.account_dir_path.is_dir() {
            fs::create_dir(self.account_dir_path.as_path())?;
        }
        Ok(())
    }

    pub fn generate_shuffle_latest_dir(&mut self) -> Result<()> {
        if !self.latest_dir_path.is_dir() {
            fs::create_dir(self.latest_dir_path.as_path())?;
        }
        Ok(())
    }

    pub fn generate_key_file(&mut self) -> Result<Ed25519PrivateKey> {
        Ok(generate_key::generate_and_save_key(
            self.latest_key_path.as_path(),
        ))
    }

    pub fn generate_address_file(&mut self, public_key: &Ed25519PublicKey) -> Result<()> {
        let address = AuthenticationKey::ed25519(public_key).derived_address();
        let address_filepath = self.latest_address_path.as_path();
        let mut file = File::create(address_filepath)?;
        file.write_all(address.to_string().as_ref())?;
        Ok(())
    }

    pub fn confirm_user_decision(&mut self) -> Result<bool> {
        let key_path = self.latest_key_path.as_path();
        let prev_key = generate_key::load_key(&key_path);
        println!(
            "Private Key already exists: {}",
            ::hex::encode(prev_key.to_bytes())
        );
        println!("Are you sure you want to generate a new key? [y/n]");

        let mut user_response = String::new();
        io::stdin()
            .read_line(&mut user_response)
            .expect("Failed to read line");
        let user_response = user_response.trim().to_owned();

        if user_response != "y" && user_response != "n" {
            println!("Please restart and enter either y or n");
            return Ok(false);
        } else if user_response == "n" {
            return Ok(false);
        }

        Ok(true)
    }
}

/// Generates the typescript bindings for the main Move package based on the embedded
/// diem types and Move stdlib. Mimics much of the transaction_builder_generator's CLI
/// except with typescript defaults and embedded content, as opposed to repo directory paths.
pub fn generate_typescript_libraries(project_path: &Path) -> Result<()> {
    let _compiled_package = deploy::build_move_packages(project_path)?;

    let pkg_path = project_path.join(MAIN_PKG_PATH);
    let target_dir = pkg_path.join("generated");
    let installer = serdegen::typescript::Installer::new(target_dir.clone());
    generate_runtime(&installer)?;
    generate_transaction_builders(&pkg_path, &target_dir)?;
    Ok(())
}

fn generate_runtime(installer: &serdegen::typescript::Installer) -> Result<()> {
    installer
        .install_serde_runtime()
        .map_err(|e| anyhow::anyhow!("unable to install Serde runtime: {:?}", e))?;
    installer
        .install_bcs_runtime()
        .map_err(|e| anyhow::anyhow!("unable to install BCS runtime: {:?}", e))?;

    // diem types
    let diem_types_content = String::from_utf8_lossy(include_bytes!(
        "../../../testsuite/generate-format/tests/staged/diem.yaml"
    ));
    let mut registry = serde_yaml::from_str::<Registry>(diem_types_content.as_ref())?;
    buildgen::typescript::replace_keywords(&mut registry);

    let config = serdegen::CodeGeneratorConfig::new("diemTypes".to_string())
        .with_encodings(vec![serdegen::Encoding::Bcs]);
    installer
        .install_module(&config, &registry)
        .map_err(|e| anyhow::anyhow!("unable to install typescript diem types: {:?}", e))?;
    Ok(())
}

fn generate_transaction_builders(pkg_path: &Path, target_dir: &Path) -> Result<()> {
    let module_name = "diemStdlib";
    let abi_directory = pkg_path;
    let abis = buildgen::read_abis(&[abi_directory])?;

    let installer: buildgen::typescript::Installer =
        buildgen::typescript::Installer::new(PathBuf::from(target_dir));
    installer
        .install_transaction_builders(module_name, abis.as_slice())
        .map_err(|e| anyhow::anyhow!("unable to install transaction builders: {:?}", e))?;
    Ok(())
}

#[cfg(test)]
mod test {
    use crate::{new, shared::Home};
    use diem_crypto::PrivateKey;
    use diem_infallible::duration_since_epoch;
    use std::fs;
    use tempfile::tempdir;

    use super::{generate_typescript_libraries, get_shuffle_project_path};

    #[test]
    fn test_get_shuffle_project_path() {
        let tmpdir = tempdir().unwrap();
        let dir_path = tmpdir.path();

        std::fs::create_dir_all(dir_path.join("nested")).unwrap();
        std::fs::write(dir_path.join("Shuffle.toml"), "goodday").unwrap();

        let actual = get_shuffle_project_path(dir_path.join("nested").as_path()).unwrap();
        let expectation = dir_path;
        assert_eq!(&actual, expectation);
    }

    #[test]
    fn test_home_create_archive_dir() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join(".shuffle/accounts")).unwrap();

        let mut home = Home::new(dir.path()).unwrap();
        let time = duration_since_epoch();
        home.create_archive_dir(time).unwrap();
        let test_archive_dir = dir
            .path()
            .join(".shuffle/accounts")
            .join(time.as_secs().to_string());
        assert_eq!(test_archive_dir.is_dir(), true);
    }

    #[test]
    fn test_home_archive_old_key() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join(".shuffle/accounts/latest")).unwrap();

        let mut home = Home::new(dir.path()).unwrap();
        let private_key = home.generate_key_file().unwrap();

        let time = duration_since_epoch();
        let archived_dir = home.create_archive_dir(time).unwrap();
        home.archive_old_key(&archived_dir).unwrap();
        let test_archive_key_path = dir
            .path()
            .join(".shuffle/accounts")
            .join(time.as_secs().to_string())
            .join("dev.key");
        let archived_key = generate_key::load_key(test_archive_key_path);

        assert_eq!(private_key, archived_key);
    }

    #[test]
    fn test_home_archive_old_address() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join(".shuffle/accounts/latest")).unwrap();

        let mut home = Home::new(dir.path()).unwrap();
        let private_key = home.generate_key_file().unwrap();
        home.generate_address_file(&private_key.public_key())
            .unwrap();
        let address_path = dir.path().join(".shuffle/accounts/latest/address");

        let time = duration_since_epoch();
        let archived_dir = home.create_archive_dir(time).unwrap();
        home.archive_old_address(&archived_dir).unwrap();
        let test_archive_address_path = dir
            .path()
            .join(".shuffle/accounts")
            .join(time.as_secs().to_string())
            .join("address");

        let old_address = fs::read_to_string(address_path).unwrap();
        let archived_address = fs::read_to_string(test_archive_address_path).unwrap();

        assert_eq!(old_address, archived_address);
    }

    #[test]
    fn test_generate_shuffle_accounts_dir() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join(".shuffle")).unwrap();

        let mut home = Home::new(dir.path()).unwrap();
        home.generate_shuffle_accounts_dir().unwrap();
        assert_eq!(dir.path().join(".shuffle/accounts").is_dir(), true);
    }

    #[test]
    fn test_generate_shuffle_latest_dir() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join(".shuffle/accounts")).unwrap();

        let mut home = Home::new(dir.path()).unwrap();
        home.generate_shuffle_latest_dir().unwrap();
        assert_eq!(dir.path().join(".shuffle/accounts/latest").is_dir(), true);
    }

    #[test]
    fn test_generate_key_file() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join(".shuffle/accounts/latest")).unwrap();

        let mut home = Home::new(dir.path()).unwrap();
        home.generate_key_file().unwrap();
        assert_eq!(
            dir.path().join(".shuffle/accounts/latest/dev.key").exists(),
            true
        );
    }

    #[test]
    fn test_generate_address_file() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join(".shuffle/accounts/latest")).unwrap();

        let mut home = Home::new(dir.path()).unwrap();
        let public_key = home.generate_key_file().unwrap().public_key();
        home.generate_address_file(&public_key).unwrap();
        assert_eq!(
            dir.path().join(".shuffle/accounts/latest/address").exists(),
            true
        );
    }

    #[test]
    fn test_home_get_shuffle_dir() {
        let dir = tempdir().unwrap();
        let home = Home::new(dir.path()).unwrap();
        let correct_dir = dir.path().join(".shuffle");
        assert_eq!(correct_dir, home.shuffle_path);
    }

    #[test]
    #[ignore]
    // Tests if the generated typesript libraries can actually be run by deno runtime.
    // `ignore`d tests are still run on CI via codegen-unit-test, but help keep
    // the local testsuite fast for devs.
    fn test_generate_typescript_libraries() {
        let tmpdir = tempdir().unwrap();
        let dir_path = tmpdir.path();
        new::write_example_move_packages(dir_path).expect("unable to create move main pkg");
        generate_typescript_libraries(dir_path).expect("unable to generate TS libraries");

        let script_path = dir_path.join("main/generated/diemStdlib/mod.ts");
        let output = std::process::Command::new("deno")
            .args(["run", script_path.to_string_lossy().as_ref()])
            .output()
            .unwrap();
        assert!(output.status.success());

        let script_contents = std::fs::read(script_path.to_string_lossy().as_ref()).unwrap();
        assert!(String::from_utf8_lossy(script_contents.as_ref())
            .contains("static encodeSetMessageScript(message: Uint8Array): DiemTypes.Script"));
    }

    #[test]
    fn test_home_get_accounts_dir() {
        let dir = tempdir().unwrap();
        let home = Home::new(dir.path()).unwrap();
        let correct_dir = dir.path().join(".shuffle/accounts");
        assert_eq!(correct_dir, home.account_dir_path);
    }

    #[test]
    fn test_home_get_latest_dir() {
        let dir = tempdir().unwrap();
        let home = Home::new(dir.path()).unwrap();
        let correct_dir = dir.path().join(".shuffle/accounts/latest");
        assert_eq!(correct_dir, home.latest_dir_path);
    }

    #[test]
    fn test_home_get_nodeconfig_path() {
        let dir = tempdir().unwrap();
        let home = Home::new(dir.path()).unwrap();
        let correct_dir = dir.path().join(".shuffle/nodeconfig/0/node.yaml");
        assert_eq!(correct_dir, home.node_config_path);
    }

    #[test]
    fn test_home_get_root_key_path() {
        let dir = tempdir().unwrap();
        let home = Home::new(dir.path()).unwrap();
        let correct_dir = dir.path().join(".shuffle/nodeconfig/mint.key");
        assert_eq!(correct_dir, home.root_key_path);
    }

    #[test]
    fn test_home_get_latest_key_path() {
        let dir = tempdir().unwrap();
        let home = Home::new(dir.path()).unwrap();
        let correct_dir = dir.path().join(".shuffle/accounts/latest/dev.key");
        assert_eq!(correct_dir, home.latest_key_path);
    }

    #[test]
    fn test_home_get_latest_address_path() {
        let dir = tempdir().unwrap();
        let home = Home::new(dir.path()).unwrap();
        let correct_dir = dir.path().join(".shuffle/accounts/latest/address");
        assert_eq!(correct_dir, home.latest_address_path);
    }
}
