use postgres::{SimpleQueryMessage, SimpleQueryRow};
use serde::Serialize;

pub fn serialize_simple(result: Vec<SimpleQueryMessage>) -> serde_json::error::Result<Vec<u8>> {
    use serde::ser::SerializeSeq;
    use serde::Serializer;

    let mut buf = Vec::new();

    let mut serializer = serde_json::Serializer::new(&mut buf);

    let mut seq = serializer.serialize_seq(None)?;
    for query in result {
        match query {
            SimpleQueryMessage::Row(row) => {
                seq.serialize_element(&Row(row))?;
            }
            SimpleQueryMessage::CommandComplete(count) => {
                seq.serialize_element(&count)?;
            }
            _ => (),
        }
    }
    seq.end()?;

    Ok(buf)
}

struct Row(SimpleQueryRow);

impl serde::ser::Serialize for Row {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::ser::Serializer,
    {
        use serde::ser::SerializeMap;
        let mut map = serializer.serialize_map(None)?;

        for (i, column) in self.0.columns().iter().enumerate() {
            if column.name() == "?column?" {
                map.serialize_key(&format!("column_{i}"))?;
            } else {
                map.serialize_key(column.name())?;
            }
            map.serialize_value(&self.0.get(i))?;
        }

        map.end()
    }
}

#[derive(Serialize)]
pub struct Notification<'a> {
    pub channel: &'a str,
    pub payload: &'a str,
}
