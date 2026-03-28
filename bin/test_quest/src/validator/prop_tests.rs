use std::collections::HashMap;
use std::hash::DefaultHasher;
use std::hash::Hash;
use std::hash::Hasher;

use rand::Rng;
use rand::SeedableRng;
use rand::rngs::StdRng;

use crate::validator::BeforeEach;
use crate::validator::ValidatedTests;

#[derive(Debug)]
pub struct PropTests {
    pub name: String,
    pub props: Option<HashMap<i32, HashMap<String, Value>>>,
    pub before_run: Option<BeforeEach>,
    pub steps: Vec<ValidatedTests>,
    pub n_runs: i32,
}

#[derive(Clone, Debug)]
pub enum Value {
    String(String),
    Number(f64),
    Array(Vec<Value>),
    Map(HashMap<Value, Value>),
}

pub fn parse_prop_tests(
    prop: Vec<(HashMap<String, String>, PropTests)>,
) -> Result<Vec<PropTests>, String /* Fix error handling */> {
    let mut global: HashMap<i32, HashMap<String, Value>> = HashMap::new();

    Ok(prop
        .iter()
        .map(|(t, p)| {
            for i in 0..p.n_runs {
                let combined = format!("{}:{}", p.name, i);
                let seed = seed_from_str(&combined);
                let mut rng = StdRng::seed_from_u64(seed);
                generate_values(&t, i, &mut global, &mut rng);
            }

            PropTests {
                name: p.name.clone(),
                props: Some(global.clone()),
                before_run: p.before_run.clone(),
                steps: p.steps.clone(),
                n_runs: p.n_runs,
            }
        })
        .collect::<Vec<_>>())
}

fn generate_values(
    props: &HashMap<String, String>,
    iteration: i32,
    global: &mut HashMap<i32, HashMap<String, Value>>,
    rng: &mut StdRng,
) {
    let mut map = HashMap::new();
    props.iter().for_each(|(key, value)| {
        let value = generate_rnd_value(value, rng);

        map.insert(key.clone(), value);
        global.insert(iteration, map.clone());
    });
}

fn generate_rnd_value(value: &str, rng: &mut StdRng) -> Value {
    match value {
        v if v.starts_with("string") => Value::String(random_string(rng)),
        v if v.starts_with("int") => {
            let n: f64 = rng.random_range(0.0..1000.0);
            Value::Number(n)
        }
        _ => {
            todo!();
        }
    }
}

fn seed_from_str(s: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    s.hash(&mut hasher);
    hasher.finish()
}

fn random_string(rng: &mut StdRng) -> String {
    let charset = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789!@#$%^&*()_+=-]}[{|/?.,\";:";
    (0..20)
        .map(|_| charset[rng.random_range(0..charset.len())] as char)
        .collect()
}

#[cfg(test)]
mod test {
    #[test]
    fn i() {}

    #[test]
    fn r() {}
}
