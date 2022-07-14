use crate::vm::errors::exec_scope_errors::ExecScopeError;
use num_bigint::BigInt;
use std::collections::HashMap;

#[derive(Eq, PartialEq, Debug)]
pub struct ExecutionScopes {
    exec_scopes: Vec<HashMap<String, PyValueType>>,
}

#[derive(Eq, Hash, PartialEq, Debug)]
pub enum PyValueType {
    BigInt(BigInt),
}

impl ExecutionScopes {
    pub fn new() -> ExecutionScopes {
        ExecutionScopes {
            exec_scopes: vec![HashMap::new()],
        }
    }

    pub fn enter_scope(&mut self, new_scope_locals: HashMap<String, PyValueType>) {
        self.exec_scopes.push(new_scope_locals);
    }

    pub fn exit_scope(&mut self) -> Result<(), ExecScopeError> {
        if self.exec_scopes.len() == 1 {
            return Err(ExecScopeError::ExitMainScopeError);
        }
        self.exec_scopes.pop();

        Ok(())
    }

    pub fn get_local_variables(&mut self) -> Option<&mut HashMap<String, PyValueType>> {
        self.exec_scopes.last_mut()
    }

    pub fn assign_or_update_variable(&mut self, var_name: &str, var_value: PyValueType) {
        if let Some(local_variables) = self.get_local_variables() {
            local_variables.insert(var_name.to_string(), var_value);
        }
    }

    pub fn delete_variable(&mut self, var_name: &str) {
        if let Some(local_variables) = self.get_local_variables() {
            local_variables.remove(&var_name.to_string());
        }
    }
}

impl Default for ExecutionScopes {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bigint;
    use num_traits::FromPrimitive;

    #[test]
    fn initialize_execution_scopes() {
        let scopes = ExecutionScopes::new();

        assert_eq!(
            scopes,
            ExecutionScopes {
                exec_scopes: vec![HashMap::new()]
            }
        );
    }

    #[test]
    fn get_local_variables_test() {
        let var_name = String::from("a");
        let var_value = PyValueType::BigInt(bigint!(2));

        let scope = HashMap::from([(var_name, var_value)]);

        let mut scopes = ExecutionScopes {
            exec_scopes: vec![scope],
        };

        assert_eq!(
            scopes.get_local_variables().unwrap(),
            &HashMap::from([(String::from("a"), PyValueType::BigInt(bigint!(2)))])
        );
    }

    #[test]
    fn enter_new_scope_test() {
        let var_name = String::from("a");
        let var_value = PyValueType::BigInt(bigint!(2));

        let new_scope = HashMap::from([(var_name, var_value)]);

        let mut scopes = ExecutionScopes {
            exec_scopes: vec![HashMap::from([(
                String::from("b"),
                PyValueType::BigInt(bigint!(1)),
            )])],
        };

        assert_eq!(
            scopes.get_local_variables().unwrap(),
            &HashMap::from([(String::from("b"), PyValueType::BigInt(bigint!(1)))])
        );

        scopes.enter_scope(new_scope);

        // check that variable `b` can't be accessed now
        assert!(scopes.get_local_variables().unwrap().get("b").is_none());

        assert_eq!(
            scopes.get_local_variables().unwrap(),
            &HashMap::from([(String::from("a"), PyValueType::BigInt(bigint!(2)))])
        );
    }

    #[test]
    fn exit_scope_test() {
        let var_name = String::from("a");
        let var_value = PyValueType::BigInt(bigint!(2));

        let new_scope = HashMap::from([(var_name, var_value)]);

        // this initializes an empty main scope
        let mut scopes = ExecutionScopes::new();

        // enter one extra scope
        scopes.enter_scope(new_scope);

        assert_eq!(
            scopes.get_local_variables().unwrap(),
            &HashMap::from([(String::from("a"), PyValueType::BigInt(bigint!(2)))])
        );

        // exit the current scope
        let exit_scope_result = scopes.exit_scope();

        assert!(exit_scope_result.is_ok());

        // assert that we recovered the older scope
        assert_eq!(scopes.get_local_variables().unwrap(), &HashMap::new());
    }

    #[test]
    fn assign_local_variable_test() {
        let var_value = PyValueType::BigInt(bigint!(2));

        let mut scopes = ExecutionScopes::new();

        scopes.assign_or_update_variable("a", var_value);

        assert_eq!(
            scopes.get_local_variables().unwrap().get("a").unwrap(),
            &PyValueType::BigInt(bigint!(2))
        );
    }

    #[test]
    fn re_assign_local_variable_test() {
        let var_name = String::from("a");
        let var_value = PyValueType::BigInt(bigint!(2));

        let scope = HashMap::from([(var_name, var_value)]);

        let mut scopes = ExecutionScopes {
            exec_scopes: vec![scope],
        };

        scopes.assign_or_update_variable("a", PyValueType::BigInt(bigint!(3)));

        assert_eq!(
            scopes.get_local_variables().unwrap().get("a").unwrap(),
            &PyValueType::BigInt(bigint!(3))
        );
    }

    #[test]
    fn delete_local_variable_test() {
        let var_name = String::from("a");
        let var_value = PyValueType::BigInt(bigint!(2));

        let scope = HashMap::from([(var_name, var_value)]);

        let mut scopes = ExecutionScopes {
            exec_scopes: vec![scope],
        };

        assert!(scopes
            .get_local_variables()
            .unwrap()
            .contains_key(&String::from("a")));

        scopes.delete_variable("a");

        assert!(!scopes
            .get_local_variables()
            .unwrap()
            .contains_key(&String::from("a")));
    }
}
