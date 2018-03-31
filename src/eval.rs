use bytecode::{Bytecode, BytecodeEngine};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub enum Value {
    U64(u64),
    Bool(bool),
    Error,
    Void,
}

fn eval_bytecode(bc: &BytecodeEngine, bytecode: &Vec<Bytecode>) -> Value {
    let mut value_stack: Vec<Value> = vec![];
    let mut var_lookup: HashMap<usize, usize> = HashMap::new();

    for code in bytecode {
        match code {
            Bytecode::ReturnVoid => {
                return Value::Void;
            }
            Bytecode::ReturnLastStackValue => match value_stack.pop() {
                Some(s) => return s,
                _ => return Value::Error,
            },
            Bytecode::Add => match (value_stack.pop(), value_stack.pop()) {
                (Some(Value::U64(rhs)), Some(Value::U64(lhs))) => {
                    value_stack.push(Value::U64(lhs + rhs));
                }
                (x, y) => unimplemented!("Can't add values of {:?} and {:?}", x, y),
            },
            Bytecode::Sub => match (value_stack.pop(), value_stack.pop()) {
                (Some(Value::U64(rhs)), Some(Value::U64(lhs))) => {
                    value_stack.push(Value::U64(lhs - rhs));
                }
                (x, y) => unimplemented!("Can't add values of {:?} and {:?}", x, y),
            },
            Bytecode::Mul => match (value_stack.pop(), value_stack.pop()) {
                (Some(Value::U64(rhs)), Some(Value::U64(lhs))) => {
                    value_stack.push(Value::U64(lhs * rhs));
                }
                (x, y) => unimplemented!("Can't add values of {:?} and {:?}", x, y),
            },
            Bytecode::Div => match (value_stack.pop(), value_stack.pop()) {
                (Some(Value::U64(rhs)), Some(Value::U64(lhs))) => {
                    value_stack.push(Value::U64(lhs / rhs));
                }
                (x, y) => unimplemented!("Can't add values of {:?} and {:?}", x, y),
            },
            Bytecode::PushU64(val) => {
                value_stack.push(Value::U64(*val));
            }
            Bytecode::PushBool(val) => {
                value_stack.push(Value::Bool(*val));
            }
            Bytecode::VarDecl(var_id) => {
                var_lookup.insert(*var_id, value_stack.len() - 1);
            }
            Bytecode::Var(var_id) => {
                let pos: usize = var_lookup[var_id];
                value_stack.push(value_stack[pos].clone());
            }
            Bytecode::Call(fn_name) => {
                let target_bytecode = bc.get_fn(fn_name);
                let result = eval_bytecode(bc, &target_bytecode.bytecode);
                value_stack.push(result);
            }
        }
    }

    Value::Void
}

pub fn eval_engine(bc: &mut BytecodeEngine, starting_fn_name: &str) -> Value {
    // begin evaluating with the first function
    let fn_bytecode = bc.get_fn(starting_fn_name);

    eval_bytecode(bc, &fn_bytecode.bytecode)
}
