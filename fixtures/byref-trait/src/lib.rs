/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */
use std::sync::Arc;

#[uniffi::export]
pub trait ByrefButton: Send + Sync {
    fn name(&self, byref: &Other) -> String;
}

#[derive(uniffi::Record)]
pub struct Other {
    pub num: u32,
}

#[derive(uniffi::Object)]
pub struct BackButton {}

#[uniffi::export]
impl BackButton {
    #[uniffi::constructor]
    pub fn new() -> Arc<Self> {
        Arc::new(Self{})
    }
}

#[uniffi::export]
impl ByrefButton for BackButton {
    fn name(&self, byref: &Other) -> String {
        format!("back{}", byref.num)
    }
}

uniffi::setup_scaffolding!();
