mod data;

use data::ScopeData;
use crate::errors::{error, CrushResult};
use std::error::Error;
use std::path::Path;
use std::sync::{Arc, Mutex};
use crate::lang::{Value, ValueType};
use std::collections::HashMap;


/**
  This is where we store variables, including functions.

  The data is protected by a mutex, in order to make sure that all threads can read and write
  concurrently.

  The data is protected by an Arc, in order to make sure that it gets deallocated.

  In order to ensure that there are no deadlocks, we only ever take one mutex at a time. This
  forces us to manually drop some references and overall write some wonky code.
*/
#[derive(Clone)]
#[derive(Debug)]
pub struct Scope {
    data: Arc<Mutex<ScopeData>>,
}

impl Scope {
    pub fn new() -> Scope {
        Scope {
            data: Arc::from(Mutex::new(ScopeData::new(None, None, false))),
        }
    }

    pub fn create_child(&self, caller: &Scope, is_loop: bool) -> Scope {
        Scope {
            data: Arc::from(Mutex::new(ScopeData::new(
                Some(self.data.clone()),
                Some(caller.data.clone()),
                is_loop))),
        }
    }

    pub fn do_break(&self) -> bool {
        do_break(&self.data)
    }

    pub fn do_continue(&self) -> bool {
        do_continue(&self.data)
    }

    pub fn is_stopped(&self) -> bool {
        self.data.lock().unwrap().is_stopped
    }

    pub fn create_namespace(&self, name: &str) -> CrushResult<Scope> {
        let res = Scope {
            data: Arc::from(Mutex::new(ScopeData::new(None, None, false))),
        };
        self.declare(&[Box::from(name)], Value::Env(res.clone()))?;
        Ok(res)
    }

    pub fn declare_str(&self, name: &str, value: Value) -> CrushResult<()> {
        let n = &name.split('.').map(|e: &str| Box::from(e)).collect::<Vec<Box<str>>>()[..];
        return self.declare(n, value);
    }

    pub fn declare(&self, name: &[Box<str>], value: Value) -> CrushResult<()> {
        if name.is_empty() {
            return error("Empty variable name");
        }
        if name.len() == 1 {
            let mut data = self.data.lock().unwrap();
            if data.is_readonly {
                return error("Scope is read only");
            }
            if data.data.contains_key(name[0].as_ref()) {
                return error(format!("Variable ${{{}}} already exists", name[0]).as_str());
            }
            data.data.insert(name[0].to_string(), value);
            Ok(())
        } else {
            match get_from_data(&self.data, name[0].as_ref()) {
                None => error("Not a namespace"),
                Some(Value::Env(env)) => env.declare(&name[1..name.len()], value),
                _ => error("Unknown namespace"),
            }
        }
    }

    pub fn set_str(&self, name: &str, value: Value) -> CrushResult<()> {
        let n = &name.split('.').map(|e: &str| Box::from(e)).collect::<Vec<Box<str>>>()[..];
        return self.set(n, value);
    }

    pub fn set(&self, name: &[Box<str>], value: Value) -> CrushResult<()> {
        if name.is_empty() {
            return error("Empty variable name");
        }
        if name.len() == 1 {
            set_on_data(&self.data, name[0].as_ref(), value)
        } else {
            match get_from_data(&self.data, name[0].as_ref()) {
                None => error("Not a namespace"),
                Some(Value::Env(env)) => env.set(&name[1..name.len()], value),
                _ => error("Unknown namespace"),
            }
        }
    }

    pub fn remove_str(&self, name: &str) -> Option<Value> {
        let n = &name.split('.').map(|e: &str| Box::from(e)).collect::<Vec<Box<str>>>()[..];
        return self.remove(n);
    }

    pub fn remove(&self, name: &[Box<str>]) -> Option<Value> {
        if name.is_empty() {
            return None;
        }
        if name.len() == 1 {
            remove_from_data(&self.data, name[0].as_ref())
        } else {
            match get_from_data(&self.data, name[0].as_ref()) {
                None => None,
                Some(Value::Env(env)) => env.remove(&name[1..name.len()]),
                _ => None,
            }
        }
    }

    pub fn get_str(&self, name: &str) -> Option<Value> {
        let n = &name.split('.').map(|e: &str| Box::from(e)).collect::<Vec<Box<str>>>()[..];
        return self.get(n);
    }

    pub fn get(&self, name: &[Box<str>]) -> Option<Value> {
        if name.is_empty() {
            return None;
        }
        if name.len() == 1 {
            get_from_data(&self.data, name[0].as_ref())
        } else {
            match get_from_data(&self.data, name[0].as_ref()) {
                None => None,
                Some(Value::Env(env)) => env.get(&name[1..name.len()]),
                _ => None,
            }
        }
    }

    pub fn uses(&self, other: &Scope) {
        self.data.lock().unwrap().uses.push(other.data.clone());
    }

    fn get_location(&self, name: &[Box<str>]) -> Option<(Scope, Vec<Box<str>>)> {
        if name.is_empty() {
            return None;
        }
        if name.len() == 1 {
            Some((self.clone(), name.to_vec()))
        } else {
            match get_from_data(&self.data, name[0].as_ref()) {
                None => None,
                Some(Value::Env(env)) => env.get_location(&name[1..name.len()]),
                _ => None,
            }
        }
    }

    pub fn dump(&self, map: &mut HashMap<String, ValueType>) {
        dump_data(&self.data, map);
    }

    pub fn readonly(&self) {
        self.data.lock().unwrap().is_readonly = true;
    }

    pub fn to_string(&self) -> String {
        let mut map = HashMap::new();
        self.dump(&mut map);
        map.iter().map(|(k, v)| k.clone()).collect::<Vec<String>>().join(", ")
    }
}

fn remove_from_data(data: &Arc<Mutex<ScopeData>>, key: &str) -> Option<Value> {
    let mut data = data.lock().unwrap();
    if !data.data.contains_key(key) {
        match data.parent_scope.clone() {
            Some(p) => {
                drop(data);
                remove_from_data(&p, key)
            }
            None => None,
        }
    } else {
        if data.is_readonly {
            return None;
        }
        data.data.remove(key)
    }
}

fn get_from_data(data: &Arc<Mutex<ScopeData>>, name: &str) -> Option<Value> {
    let data = data.lock().unwrap();
    match data.data.get(&name.to_string()) {
        Some(v) => Some(v.clone()),
        None => match data.parent_scope.clone() {
            Some(p) => {
                drop(data);
                get_from_data(&p, name)
            },
            None => {
                let uses = data.uses.clone();
                drop(data);
                for ulock in &uses {
                    if let Some(res) = get_from_data(ulock, name) {
                        return Some(res);
                    }
                }
                None
            }
        }
    }
}

fn set_on_data(data: &Arc<Mutex<ScopeData>>, name: &str, value: Value) -> CrushResult<()> {
    let mut data = data.lock().unwrap();
    if !data.data.contains_key(name) {
        match data.parent_scope.clone() {
            Some(p) => {
                drop(data);
                set_on_data(&p, name, value)
            }
            None => error(format!("Unknown variable ${{{}}}", name).as_str()),
        }
    } else {
        if data.is_readonly {
            error("Scope is read only")
        } else if data.data[name].value_type() != value.value_type() {
            error(format!("Type mismatch when reassigning variable ${{{}}}. Use `unset ${{{}}}` to remove old variable.", name, name).as_str())
        } else {
            data.data.insert(name.to_string(), value);
            Ok(())
        }
    }
}

/**
    This function takes the lock twice. We could avoid that with a bit of extra copying, not sure if
    that would be an improvement.
*/
fn dump_data(data: &Arc<Mutex<ScopeData>>, map: &mut HashMap<String, ValueType>) {
    match data.lock().unwrap().parent_scope.clone() {
        Some(p) => dump_data(&p, map),
        None => {}
    }

    let data = data.lock().unwrap();
    for (k, v) in data.data.iter() {
        map.insert(k.clone(), v.value_type());
    }
}

fn do_continue(shared: &Arc<Mutex<ScopeData>>) -> bool {
    let data = shared.lock().unwrap();
    if data.is_readonly {
        return false;
    } else if data.is_loop {
        true
    } else {
        let caller = data.calling_scope.clone();
        drop(data);
        let ok = caller
            .map(|p| do_continue(&p))
            .unwrap_or(false);
        if !ok {
            false
        } else {
            shared.lock().unwrap().is_stopped = true;
            true
        }
    }
}

pub fn do_break(shared: &Arc<Mutex<ScopeData>>) -> bool {
    let mut data = shared.lock().unwrap();
    if data.is_readonly {
        false
    } else if data.is_loop {
        data.is_stopped = true;
        true
    } else {
        let caller = data.calling_scope.clone();
        drop(data);
        let ok = caller
            .map(|p| do_break(&p))
            .unwrap_or(false);
        if !ok {
            false
        } else {
            shared.lock().unwrap().is_stopped = true;
            true
        }
    }
}


pub fn cwd() -> CrushResult<Box<Path>> {
    match std::env::current_dir() {
        Ok(d) => Ok(d.into_boxed_path()),
        Err(e) => error(e.description()),
    }
}

pub fn home() -> CrushResult<Box<Path>> {
    match dirs::home_dir() {
        Some(d) => Ok(d.into_boxed_path()),
        None => error("Could not find users home directory"),
    }
}