// Copyright (c) The Libra Core Contributors
// SPDX-License-Identifier: Apache-2.0

use crate::account_config::{AccountResource, ACCOUNT_RESOURCE_PATH};
use anyhow::{Error, Result};
use serde::export::Formatter;
use serde::{Deserialize, Serialize};
use std::collections::btree_map::BTreeMap;
use std::convert::TryFrom;
use std::fmt;

#[derive(Default, Deserialize, Serialize)]
pub struct AccountState(BTreeMap<Vec<u8>, Vec<u8>>);

impl AccountState {
    pub fn get_account_resource(&self) -> Result<Option<AccountResource>> {
        self.0
            .get(&*ACCOUNT_RESOURCE_PATH)
            .map(|bytes| lcs::from_bytes(bytes))
            .transpose()
            .map_err(Into::into)
    }

    pub fn get(&self, key: &[u8]) -> Option<&Vec<u8>> {
        self.0.get(key)
    }

    pub fn insert(&mut self, key: Vec<u8>, value: Vec<u8>) -> Option<Vec<u8>> {
        self.0.insert(key, value)
    }

    pub fn remove(&mut self, key: &[u8]) -> Option<Vec<u8>> {
        self.0.remove(key)
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl fmt::Debug for AccountState {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let account_resource_str = self
            .get_account_resource()
            .map(|account_resource_opt| format!("{:#?}", account_resource_opt))
            .unwrap_or_else(|e| format!("parse error: {:#?}", e));
        // TODO: add support for other types of resources

        write!(f, "AccountResource {{ {} }}", account_resource_str)
    }
}

impl TryFrom<&AccountResource> for AccountState {
    type Error = Error;

    fn try_from(account_resource: &AccountResource) -> Result<Self> {
        let mut btree_map: BTreeMap<Vec<u8>, Vec<u8>> = BTreeMap::new();
        btree_map.insert(
            ACCOUNT_RESOURCE_PATH.to_vec(),
            lcs::to_bytes(account_resource)?,
        );

        Ok(Self(btree_map))
    }
}
