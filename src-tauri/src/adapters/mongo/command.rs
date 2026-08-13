use mongodb::bson::{Bson, Document};

use crate::error::{Error, Result};

/// A parsed MongoDB shell-style command (`db.<collection>.<method>(...)`).
#[derive(Debug, Clone, PartialEq)]
pub enum MongoCommand {
    Find {
        collection: String,
        filter: Document,
        projection: Option<Document>,
    },
    FindOne {
        collection: String,
        filter: Document,
        projection: Option<Document>,
    },
    Aggregate {
        collection: String,
        pipeline: Vec<Document>,
    },
    CountDocuments {
        collection: String,
        filter: Document,
    },
    Distinct {
        collection: String,
        field: String,
        filter: Document,
    },
    InsertOne {
        collection: String,
        document: Document,
    },
    InsertMany {
        collection: String,
        documents: Vec<Document>,
    },
    UpdateOne {
        collection: String,
        filter: Document,
        update: Document,
    },
    UpdateMany {
        collection: String,
        filter: Document,
        update: Document,
    },
    DeleteOne {
        collection: String,
        filter: Document,
    },
    DeleteMany {
        collection: String,
        filter: Document,
    },
    Drop {
        collection: String,
    },
}

impl MongoCommand {
    pub fn is_write(&self) -> bool {
        !matches!(
            self,
            Self::Find { .. }
                | Self::FindOne { .. }
                | Self::Aggregate { .. }
                | Self::CountDocuments { .. }
                | Self::Distinct { .. }
        )
    }
}

/// Parse a shell-style command like `db.users.find({ age: { $gt: 18 } })`.
/// Arguments accept JSON5 (unquoted keys, single quotes, trailing commas).
pub fn parse_command(input: &str) -> Result<MongoCommand> {
    let trimmed = input.trim().trim_end_matches(';').trim();

    let rest = trimmed.strip_prefix("db.").ok_or_else(|| {
        Error::InvalidQuery(
            "MongoDB commands must start with 'db.', e.g. db.users.find({})".into(),
        )
    })?;

    let open_paren = rest.find('(').ok_or_else(|| {
        Error::InvalidQuery("Expected a method call, e.g. db.users.find({})".into())
    })?;

    if !rest.ends_with(')') {
        return Err(Error::InvalidQuery("Unbalanced parentheses in command".into()));
    }

    let head = &rest[..open_paren];
    let args_str = &rest[open_paren + 1..rest.len() - 1];

    let (collection, method) = head.rsplit_once('.').ok_or_else(|| {
        Error::InvalidQuery(
            "Expected 'db.<collection>.<method>(...)', e.g. db.users.find({})".into(),
        )
    })?;

    if collection.is_empty() {
        return Err(Error::InvalidQuery("Missing collection name".into()));
    }

    let args = split_top_level(args_str)?;
    let collection = collection.to_string();

    let command = match method {
        "find" => MongoCommand::Find {
            collection,
            filter: opt_doc(args.first())?,
            projection: args.get(1).map(|a| parse_doc(a)).transpose()?,
        },
        "findOne" => MongoCommand::FindOne {
            collection,
            filter: opt_doc(args.first())?,
            projection: args.get(1).map(|a| parse_doc(a)).transpose()?,
        },
        "aggregate" => MongoCommand::Aggregate {
            collection,
            pipeline: parse_doc_array(required(&args, 0, method)?)?,
        },
        "countDocuments" | "count" => MongoCommand::CountDocuments {
            collection,
            filter: opt_doc(args.first())?,
        },
        "distinct" => MongoCommand::Distinct {
            collection,
            field: parse_string(required(&args, 0, method)?)?,
            filter: opt_doc(args.get(1))?,
        },
        "insertOne" => MongoCommand::InsertOne {
            collection,
            document: parse_doc(required(&args, 0, method)?)?,
        },
        "insertMany" => MongoCommand::InsertMany {
            collection,
            documents: parse_doc_array(required(&args, 0, method)?)?,
        },
        "updateOne" => MongoCommand::UpdateOne {
            collection,
            filter: parse_doc(required(&args, 0, method)?)?,
            update: parse_doc(required(&args, 1, method)?)?,
        },
        "updateMany" => MongoCommand::UpdateMany {
            collection,
            filter: parse_doc(required(&args, 0, method)?)?,
            update: parse_doc(required(&args, 1, method)?)?,
        },
        "deleteOne" => MongoCommand::DeleteOne {
            collection,
            filter: parse_doc(required(&args, 0, method)?)?,
        },
        "deleteMany" => MongoCommand::DeleteMany {
            collection,
            filter: parse_doc(required(&args, 0, method)?)?,
        },
        "drop" => MongoCommand::Drop { collection },
        other => {
            return Err(Error::InvalidQuery(format!(
                "Unsupported method '{other}'. Supported: find, findOne, aggregate, \
                 countDocuments, distinct, insertOne, insertMany, updateOne, updateMany, \
                 deleteOne, deleteMany, drop"
            )))
        }
    };

    Ok(command)
}

// ─────────────────────────────────────────────────────────────────────────────
// Argument parsing helpers
// ─────────────────────────────────────────────────────────────────────────────

fn required<'a>(args: &'a [String], idx: usize, method: &str) -> Result<&'a str> {
    args.get(idx).map(String::as_str).ok_or_else(|| {
        Error::InvalidQuery(format!(
            "{method}() requires at least {} argument(s)",
            idx + 1
        ))
    })
}

fn opt_doc(arg: Option<&String>) -> Result<Document> {
    match arg {
        Some(a) if !a.trim().is_empty() => parse_doc(a),
        _ => Ok(Document::new()),
    }
}

fn parse_json5(arg: &str) -> Result<Bson> {
    let value: serde_json::Value = json5::from_str(arg)
        .map_err(|e| Error::InvalidQuery(format!("Invalid argument '{arg}': {e}")))?;
    Bson::try_from(value)
        .map_err(|e| Error::InvalidQuery(format!("Invalid BSON in '{arg}': {e}")))
}

fn parse_doc(arg: &str) -> Result<Document> {
    match parse_json5(arg)? {
        Bson::Document(doc) => Ok(doc),
        _ => Err(Error::InvalidQuery(format!(
            "Expected an object, got: {arg}"
        ))),
    }
}

fn parse_doc_array(arg: &str) -> Result<Vec<Document>> {
    match parse_json5(arg)? {
        Bson::Array(items) => items
            .into_iter()
            .map(|item| match item {
                Bson::Document(doc) => Ok(doc),
                other => Err(Error::InvalidQuery(format!(
                    "Expected an array of objects, found: {other}"
                ))),
            })
            .collect(),
        Bson::Document(doc) => Ok(vec![doc]),
        _ => Err(Error::InvalidQuery(format!(
            "Expected an array of objects, got: {arg}"
        ))),
    }
}

fn parse_string(arg: &str) -> Result<String> {
    match parse_json5(arg)? {
        Bson::String(s) => Ok(s),
        _ => Err(Error::InvalidQuery(format!("Expected a string, got: {arg}"))),
    }
}

/// Split a method's argument list at top-level commas, respecting nesting
/// (`{}`, `[]`, `()`) and string literals with escapes.
fn split_top_level(input: &str) -> Result<Vec<String>> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut depth: i32 = 0;
    let mut in_string: Option<char> = None;
    let mut escaped = false;

    for c in input.chars() {
        if let Some(quote) = in_string {
            current.push(c);
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == quote {
                in_string = None;
            }
            continue;
        }

        match c {
            '"' | '\'' => {
                in_string = Some(c);
                current.push(c);
            }
            '{' | '[' | '(' => {
                depth += 1;
                current.push(c);
            }
            '}' | ']' | ')' => {
                depth -= 1;
                if depth < 0 {
                    return Err(Error::InvalidQuery("Unbalanced brackets in arguments".into()));
                }
                current.push(c);
            }
            ',' if depth == 0 => {
                args.push(current.trim().to_string());
                current.clear();
            }
            _ => current.push(c),
        }
    }

    if depth != 0 || in_string.is_some() {
        return Err(Error::InvalidQuery("Unbalanced brackets or string in arguments".into()));
    }

    let last = current.trim();
    if !last.is_empty() {
        args.push(last.to_string());
    }

    Ok(args)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mongodb::bson::doc;

    #[test]
    fn parses_find_with_unquoted_keys() {
        let cmd = parse_command("db.users.find({ age: { $gt: 18 } })").unwrap();
        assert_eq!(
            cmd,
            MongoCommand::Find {
                collection: "users".into(),
                filter: doc! { "age": { "$gt": 18 } },
                projection: None,
            }
        );
    }

    #[test]
    fn parses_find_empty_args() {
        let cmd = parse_command("db.users.find()").unwrap();
        assert_eq!(
            cmd,
            MongoCommand::Find {
                collection: "users".into(),
                filter: doc! {},
                projection: None,
            }
        );
    }

    #[test]
    fn parses_find_with_projection_and_semicolon() {
        let cmd = parse_command("db.users.find({}, { name: 1 });").unwrap();
        assert_eq!(
            cmd,
            MongoCommand::Find {
                collection: "users".into(),
                filter: doc! {},
                projection: Some(doc! { "name": 1 }),
            }
        );
    }

    #[test]
    fn parses_aggregate_pipeline() {
        let cmd = parse_command(
            "db.orders.aggregate([{ $match: { total: { $gte: 10 } } }, { $limit: 5 }])",
        )
        .unwrap();
        assert_eq!(
            cmd,
            MongoCommand::Aggregate {
                collection: "orders".into(),
                pipeline: vec![
                    doc! { "$match": { "total": { "$gte": 10 } } },
                    doc! { "$limit": 5 },
                ],
            }
        );
    }

    #[test]
    fn parses_update_with_two_args() {
        let cmd =
            parse_command("db.users.updateOne({ _id: 1 }, { $set: { name: 'ana' } })").unwrap();
        assert_eq!(
            cmd,
            MongoCommand::UpdateOne {
                collection: "users".into(),
                filter: doc! { "_id": 1 },
                update: doc! { "$set": { "name": "ana" } },
            }
        );
    }

    #[test]
    fn collection_names_with_dots() {
        let cmd = parse_command("db.system.profile.find({})").unwrap();
        assert_eq!(
            cmd,
            MongoCommand::Find {
                collection: "system.profile".into(),
                filter: doc! {},
                projection: None,
            }
        );
    }

    #[test]
    fn comma_inside_string_not_split() {
        let cmd = parse_command(r#"db.users.find({ name: "a,b" })"#).unwrap();
        assert_eq!(
            cmd,
            MongoCommand::Find {
                collection: "users".into(),
                filter: doc! { "name": "a,b" },
                projection: None,
            }
        );
    }

    #[test]
    fn rejects_non_db_prefix() {
        assert!(parse_command("show dbs").is_err());
    }

    #[test]
    fn rejects_unknown_method() {
        let err = parse_command("db.users.explode({})").unwrap_err();
        assert!(err.to_string().contains("Unsupported method"));
    }

    #[test]
    fn write_detection() {
        assert!(parse_command("db.u.deleteMany({})").unwrap().is_write());
        assert!(!parse_command("db.u.find({})").unwrap().is_write());
    }
}
