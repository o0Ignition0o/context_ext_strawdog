use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::Value;
use std::any::Any;
use std::collections::HashMap;

trait JSContextExt: Sized + Serialize + DeserializeOwned {
    fn read_json(&self) -> serde_json::Value;

    fn update_json(&mut self, callback: impl Fn(Value) -> Value);
}

impl<T> JSContextExt for T
where
    T: Sized + Serialize + DeserializeOwned,
{
    fn read_json(&self) -> serde_json::Value {
        serde_json::to_value(&self).unwrap()
    }

    fn update_json(&mut self, callback: impl Fn(Value) -> Value) {
        let serialized = serde_json::to_value(&self).unwrap();

        let updated = callback(serialized);

        let deserialized: Self = serde_json::from_value(updated).unwrap();

        *self = deserialized;
    }
}

#[derive(Serialize, Deserialize, Debug)]
struct Stuff {
    foo: usize,
    bar: String,
}

#[derive(Debug)]
struct NotSerializableStuff {
    baz: usize,
}

struct Context {
    everything: HashMap<String, Box<dyn Any>>,
}

impl Context {
    pub fn default() -> Self {
        Self {
            everything: Default::default(),
        }
    }

    pub fn push(&mut self, key: String, value: Box<dyn Any>) -> Result<(), &'static str> {
        if self.everything.get(key.as_str()).is_some() {
            Err("this exists already!, use get or with instead!")
        } else {
            self.everything.insert(key, value);
            Ok(())
        }
    }

    pub fn read<'this, T: 'static>(&'this self, key: &str) -> Option<&'this T> {
        self.everything.get(key).map(|t| t.downcast_ref()).flatten()
    }

    pub fn write_with<T: 'static>(
        &mut self,
        key: &str,
        mut callback: impl FnMut(&mut T),
    ) -> Result<(), String> {
        let with_key = self
            .everything
            .get_mut(key)
            .ok_or_else(|| format!("there is no contents with key {}", key))?;
        let as_t = with_key.downcast_mut().ok_or_else(|| {
            format!(
                "value with key {} is not of expected type {}",
                key,
                std::any::type_name::<T>()
            )
        })?;
        callback(as_t);
        Ok(())
    }
}

fn main() {
    let s = Stuff {
        foo: 42,
        bar: "hello".to_string(),
    };

    dbg!("created stuff: ", &s);

    let mut ctx = Context::default();
    ctx.push("stuff".to_string(), Box::new(s)).unwrap();

    // reads will not persist to ctx
    ctx.read("stuff").map(|stuff: &Stuff| {
        let new_stuff = Stuff {
            foo: stuff.foo * 2,
            bar: stuff.bar.clone(),
        };
        dbg!("within read", &new_stuff);
    });
    // still the old stuff
    dbg!("after read", ctx.read::<Stuff>("stuff"));

    // write with will persist things to ctx
    ctx.write_with("stuff", |mut stuff: &mut Stuff| {
        stuff.bar = "this will be persisted!".to_string();

        dbg!("within write", &stuff);
    })
    .expect("oops");

    // now it s the new stuff
    dbg!("after write", ctx.read::<Stuff>("stuff"));

    // json reads can be done like this
    ctx.read::<Stuff>("stuff").map(|stuff| {
        dbg!(stuff.read_json());
    });

    // json updates can be done like this
    ctx.write_with("stuff", |stuff: &mut Stuff| {
        stuff.update_json(|value| {
            // we could poke at the json object, or do anything really
            let mut stuff: Stuff = serde_json::from_value(value).unwrap();

            stuff.foo = 1;

            serde_json::to_value(stuff).unwrap()
        });
    })
    .expect("oops");

    dbg!("after json write, foo = 1", ctx.read::<Stuff>("stuff"));

    // -------------------

    // I can also create not serializable stuff
    let ns = NotSerializableStuff { baz: 42 };

    // and insert it into the context
    ctx.push("notserializablestuff".to_string(), Box::new(ns))
        .unwrap();

    // and i ll be able to read and write
    ctx.write_with("notserializablestuff", |ns: &mut NotSerializableStuff| {
        ns.baz = 14;
    });

    dbg!(&ctx.read::<NotSerializableStuff>("notserializablestuff"));

    // but I won't be able to read / write json values, so js interop will be hard
    ctx.write_with("notserializablestuff", |ns: &mut NotSerializableStuff| {
        ns.update_json(|value| {
            // this wont compile
        });
    });

    //     error[E0599]: the method `update_json` exists for mutable reference `&mut NotSerializableStuff`, but its trait bounds were not satisfied
    //    --> src/main.rs:158:12
    //     |
    // 38  | struct NotSerializableStuff {
    //     | ---------------------------
    //     | |
    //     | doesn't satisfy `NotSerializableStuff: DeserializeOwned`
    //     | doesn't satisfy `NotSerializableStuff: JSContextExt`
    //     | doesn't satisfy `NotSerializableStuff: Serialize`
    // ...
    // 158 |         ns.update_json(|value| {
    //     |            ^^^^^^^^^^^ method cannot be called on `&mut NotSerializableStuff` due to unsatisfied trait bounds
    //     |

    dbg!(&ctx
        .read::<NotSerializableStuff>("notserializablestuff")
        .unwrap()
        .read_json());

    //         error[E0599]: the method `read_json` exists for reference `&NotSerializableStuff`, but its trait bounds were not satisfied
    //    --> src/main.rs:165:10
    //     |
    // 38  | struct NotSerializableStuff {
    //     | ---------------------------
    //     | |
    //     | doesn't satisfy `NotSerializableStuff: DeserializeOwned`
    //     | doesn't satisfy `NotSerializableStuff: JSContextExt`
    //     | doesn't satisfy `NotSerializableStuff: Serialize`
    // ...
    // 165 |         .read_json());
    //     |          ^^^^^^^^^ method cannot be called on `&NotSerializableStuff` due to unsatisfied trait bounds
    //     |
}
