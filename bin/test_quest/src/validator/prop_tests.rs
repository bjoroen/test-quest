use std::collections::HashMap;

use proptest::char::any;
use proptest::prelude::BoxedStrategy;
use proptest::prelude::Strategy;
use serde_json::Value;

use crate::parser::RawPropTests;
use crate::validator::PropTests;

pub fn parse_prop_tests(tests: &Vec<RawPropTests>) -> Result<PropTests, String> {
    todo!()
}

fn generate_strategy(
    props: HashMap<String, String>,
) -> BoxedStrategy<HashMap<String, serde_json::Value>> {
    let strategies: Vec<(String, BoxedStrategy<serde_json::Value>)> = props
        .into_iter()
        .map(|(k, v)| (k, prop_strat(&v)))
        .collect();

    strategies
        .into_iter()
        .map(|(k, strat)| strat.prop_map(move |v| (k.clone(), v)))
        .collect::<Vec<_>>()
        .prop_map(|pairs| pairs.into_iter().collect::<HashMap<_, _>>())
        .boxed()
}

fn prop_strat(def: &str) -> BoxedStrategy<serde_json::Value> {
    // parse string(min,max)
    let parts: Vec<&str> = def
        .trim_start_matches("string(")
        .trim_end_matches(")")
        .split(',')
        .collect();
    let min: usize = parts[0].parse().unwrap();
    let max: usize = parts[1].parse().unwrap();

    proptest::collection::vec(any(), min..=max)
        .prop_map(|chars| chars.into_iter().collect::<String>())
        .prop_map(serde_json::Value::String)
        .boxed()
}
/// Recursively replace any "props.xxx" string with the generated value
fn interpolate_props(body: &Value, props: &HashMap<String, Value>) -> Value {
    match body {
        Value::Object(map) => {
            let mut new_map = serde_json::Map::new();
            for (k, v) in map {
                new_map.insert(k.clone(), interpolate_props(v, props));
            }
            Value::Object(new_map)
        }
        Value::Array(arr) => {
            Value::Array(arr.iter().map(|v| interpolate_props(v, props)).collect())
        }
        Value::String(s) => {
            if let Some(prop_name) = s.strip_prefix("props.") {
                props.get(prop_name).cloned().unwrap_or(Value::Null)
            } else {
                Value::String(s.clone())
            }
        }
        other => other.clone(),
    }
}

#[cfg(test)]
mod test {
    use std::collections::HashMap;

    use proptest::*;
    use serde_json::Value;

    use crate::validator::prop_tests::generate_strategy;
    use crate::validator::prop_tests::interpolate_props;

    #[test]
    fn generate_strategy_test() {
        let mut props = HashMap::new();
        props.insert("password".to_string(), "string(1,20)".to_string());

        let strat = generate_strategy(props);

        proptest!(|(generated in strat)| {
            println!("Generated props: {:?}", generated);
        });
        panic!()
    }

    #[test]
    fn interpolate_props_test() {
        let generated_props: HashMap<String, Value> = HashMap::from([(
            "password".to_string(),
            Value::String("mysecret123".to_string()),
        )]);

        let body: Value = serde_json::json!({
            "username": "Harry Potter",
            "password": "props.password"
        });

        let concrete_body = interpolate_props(&body, &generated_props);

        assert_eq!(
            concrete_body.get("password").unwrap(),
            &serde_json::Value::String("mysecret123".to_string())
        )
    }
}
