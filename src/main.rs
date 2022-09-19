use serde_json::{json, Number, Value};

type Name = String;
type Pair = (Name, Option<Value>);
type Record = Vec<Pair>;
type Transform = fn(Option<Value>) -> Option<Value>;

#[derive(Debug)]
enum Schema<'a> {
    Sub(&'a str, Vec<Schema<'a>>),
    Key(&'a str, Option<&'a str>, Option<Transform>),
}

#[allow(dead_code)]
impl<'a> Schema<'a> {
    fn names(&self) {
        self._names("");
    }

    fn _names(&self, prefix: &str) {
        match self {
            Self::Sub(name, schema) => {
                for value in schema.iter() {
                    value._names(&Schema::prefix(prefix, name));
                }
            }
            Self::Key(name, _, _) => {
                println!("{}", Schema::prefix(prefix, name));
            }
        }
    }

    fn extract(&self, record: &Value) -> Vec<Record> {
        self._extract_sub(Some(record), "")
    }

    fn _extract_sub(&self, record: Option<&Value>, prefix: &str) -> Vec<Record> {
        match self {
            Self::Sub(name, schema) => {
                let prefix = Schema::prefix(prefix, name);

                let mut results = vec![];
                let mut fields = vec![];
                let mut subdocs = vec![];

                if let Some(record) = record {
                    for item in schema.iter() {
                        match item {
                            k @ Schema::Sub(name, _) => match record {
                                Value::Object(m) => match m.get(*name) {
                                    o @ Some(Value::Object(_)) => {
                                        subdocs.push(k._extract_sub(o, &prefix))
                                    }
                                    Some(Value::Array(arr)) => {
                                        let sub = arr
                                            .iter()
                                            .flat_map(|v| k._extract_sub(Some(v), &prefix))
                                            .collect();
                                        subdocs.push(sub);
                                    }
                                    _ => {}
                                },
                                _ => subdocs.push(k._extract_sub(None, &prefix)),
                            },
                            k @ Schema::Key(_, _, _) => {
                                fields.push(k._extract_key(Some(record), &prefix));
                            }
                        }
                    }
                }

                if fields.len() > 0 {
                    subdocs.push(vec![fields]);
                }

                results.append(&mut merge(subdocs));

                results
            }
            Self::Key(_, _, _) => panic!("Cannot call _extract_sub on Key!"),
        }
    }

    fn _extract_key(&self, record: Option<&Value>, prefix: &str) -> Pair {
        match self {
            Self::Sub(_, _) => panic!("Cannot call _extract_key on Sub!"),
            Self::Key(key, name, transform) => {
                let k = match name {
                    Some(name) => name.to_string(),
                    None => Schema::prefix(prefix, key),
                };

                let value = match record {
                    Some(Value::Object(m)) => match m.get(*key) {
                        None => None,
                        Some(v) => Some(v.clone()),
                    },
                    _ => None,
                };

                if let Some(func) = transform {
                    (k, func(value))
                } else {
                    (k, value)
                }
            }
        }
    }

    fn prefix(prefix: &'a str, name: &'a str) -> String {
        if prefix == "" {
            format!("{name}")
        } else {
            format!("{prefix}_{name}")
        }
    }
}

macro_rules! key {
    ($id:expr) => {
        Schema::Key($id, None, None)
    };
    ($id:expr, $name:expr) => {
        Schema::Key($id, Some($name), None)
    };
    ($id:expr, $name:expr, $func:expr) => {
        Schema::Key($id, Some($name), Some($func))
    };
}

macro_rules! doc {
    ($($schema:expr),+) => {
        Schema::Sub("", vec![$($schema),+])
    };
}

macro_rules! sub {
    ($id:expr, {$($schema:expr),+}) => {
        Schema::Sub($id, vec![$($schema),+])
    };
}

fn main() {
    let data = json!({
        "id": 1,
        "name": "Felix Alonso",
        "phone": {"type": "cell", "number": "661 867 5309"},
        "family": [
            {"relation": "mom", "name": "Mother Superior"},
            {"relation": "dad", "name": "Father Dearest"},
        ]
    });

    let schema = doc! {
        key!("id", "human_id", inc),
        key!("name"),
        sub!("phone", {
            key!("type"),
            key!("number")
        }),
        sub!("family", {
            key!("relation", "relationship"),
            key!("name", "full_name")
        })
    };

    let results = schema.extract(&data);

    println!("{}", results.len());
    for result in results.iter() {
        println!("{:?}", result);
    }

    println!("{:?}", results[0][0].1.as_ref().unwrap());
}

fn inc(val: Option<Value>) -> Option<Value> {
    if let Some(Value::Number(n)) = val {
        Some(Value::Number(
            Number::from_f64(n.as_f64().unwrap() + 1.0).unwrap(),
        ))
    } else {
        val
    }
}

fn merge(mut sets: Vec<Vec<Record>>) -> Vec<Record> {
    match sets.len() {
        0 => vec![],
        1 => sets[0].clone(),
        2 => merge_two(sets[0].clone(), sets[1].clone()),
        _ => {
            let head = sets.pop().unwrap();
            sets.into_iter().fold(head, merge_two)
        }
    }
}

fn merge_two(left: Vec<Record>, right: Vec<Record>) -> Vec<Record> {
    let s1 = left.into_iter();
    let s2 = right.into_iter();
    s1.clone()
        .flat_map(|x: Record| {
            s2.clone()
                .map(move |mut y: Record| {
                    y.append(&mut x.clone());
                    y.clone()
                })
                .collect::<Vec<Record>>()
        })
        .collect()
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn merge_nothing() {
        let data = vec![];
        let expected: Vec<Record> = vec![];
        assert_eq!(merge(data), expected);
    }

    #[test]
    fn merge_one() {
        let data = vec![vec![vec![("test".to_string(), None)]]];
        let expected: Vec<Record> = vec![vec![("test".to_string(), None)]];
        assert_eq!(merge(data), expected);
    }

    #[test]
    fn merge_two() {
        let first = ("test1".into(), None);
        let second = ("test2".into(), None);
        let data: Vec<Vec<Record>> = vec![vec![vec![first.clone()]], vec![vec![second.clone()]]];
        let expected: Vec<Record> = vec![vec![second, first]];
        assert_eq!(merge(data), expected);
    }

    #[test]
    fn merge_two_by_one() {
        let first = ("test1".into(), None);
        let second = ("test2".into(), None);
        let third = ("test3".into(), None);
        let data: Vec<Vec<Record>> = vec![
            vec![vec![first.clone()]],
            vec![vec![second.clone()], vec![third.clone()]],
        ];
        let expected: Vec<Record> = vec![vec![second, first.clone()], vec![third, first.clone()]];
        assert_eq!(merge(data), expected);
    }
}
