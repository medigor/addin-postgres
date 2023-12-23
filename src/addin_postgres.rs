#![allow(unused_imports)]
#![allow(unused_variables)]
#![allow(dead_code)]

use std::{
    borrow::Borrow,
    error::{self, Error},
    fmt::Display,
    time::Duration,
};

use addin1c::{name, AddinError, MethodInfo, Methods, ParamValue, PropInfo, SimpleAddin, Variant};
use postgres::{fallible_iterator::FallibleIterator, notifications, Client, NoTls};
use serde::Serialize;
use smallvec::SmallVec;
use utf16_lit::utf16;

use crate::serializer::{self, serialize_simple};

type AddinResult = Result<(), Box<dyn std::error::Error>>;

pub struct Addin {
    client: Option<Client>,
    last_error: Option<Box<dyn Error>>,
}

impl Addin {
    pub fn new() -> Self {
        Self {
            client: None,
            last_error: None,
        }
    }

    fn last_error(&mut self, value: &mut Variant) -> AddinError {
        match &self.last_error {
            Some(err) => value
                .set_str1c(err.to_string().as_str())
                .map_err(|e| e.into()),
            None => value.set_str1c("").map_err(|e| e.into()),
        }
    }

    fn connect(&mut self, connection_string: &mut Variant, ret_value: &mut Variant) -> AddinResult {
        let connection_string = connection_string.get_string()?;
        if self.client.is_some() {
            return Ok(());
        }
        let client = Client::connect(&connection_string, NoTls)?;
        self.client = Some(client);
        Ok(())
    }

    fn client(&mut self) -> Result<&mut Client, Box<dyn Error>> {
        let Some(client) = &mut self.client else {
            return Err("Not connected".into());
        };
        Ok(client)
    }

    fn connected(&mut self, value: &mut Variant) -> AddinResult {
        let Some(client) = &self.client else {
            value.set_bool(false);
            return Ok(());
        };
        value.set_bool(!client.is_closed());
        Ok(())
    }

    fn simple_query(&mut self, sql: &mut Variant, ret_value: &mut Variant) -> AddinResult {
        let client = self.client()?;
        let sql = sql.get_string()?;
        let result = client.simple_query(&sql)?;
        let result = serialize_simple(result)?;
        ret_value.set_blob(&result)?;
        Ok(())
    }

    fn notifications(&mut self, timeout: &mut Variant, ret_value: &mut Variant) -> AddinResult {
        use serde::ser::SerializeSeq;
        use serde::Serializer;

        let client = self.client()?;
        let timeout = timeout.get_i32()?;
        if timeout < 0 {
            return Err("Timeout must be >= 0".into());
        }

        let mut buf = Vec::new();
        let mut serializer = serde_json::Serializer::new(&mut buf);
        let mut seq = serializer.serialize_seq(None)?;

        let mut notifications = client.notifications();

        if timeout > 0 {
            if let Some(notification) = notifications
                .timeout_iter(Duration::from_millis(timeout as u64))
                .next()?
            {
                seq.serialize_element(&serializer::Notification {
                    channel: notification.channel(),
                    payload: notification.payload(),
                })?;
            };
        }

        for notification in notifications.iter().iterator() {
            let notification = notification?;
            seq.serialize_element(&serializer::Notification {
                channel: notification.channel(),
                payload: notification.payload(),
            })?;
        }

        seq.end()?;
        ret_value.set_blob(&buf)?;

        Ok(())
    }
}

impl SimpleAddin for Addin {
    fn name() -> &'static [u16] {
        name!("Postgres")
    }

    fn save_error(&mut self, err: Option<Box<dyn Error>>) {
        self.last_error = err;
    }

    fn methods() -> &'static [MethodInfo<Self>] {
        &[
            MethodInfo {
                name: name!("Connect"),
                method: Methods::Method1(Self::connect),
            },
            MethodInfo {
                name: name!("SimpleQuery"),
                method: Methods::Method1(Self::simple_query),
            },
            MethodInfo {
                name: name!("Notifications"),
                method: Methods::Method1(Self::notifications),
            },
        ]
    }

    fn properties() -> &'static [PropInfo<Self>] {
        &[
            PropInfo {
                name: name!("Connected"),
                getter: Some(Self::connected),
                setter: None,
            },
            PropInfo {
                name: name!("LastError"),
                getter: Some(Self::last_error),
                setter: None,
            },
        ]
    }
}
