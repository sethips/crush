use crate::lib::ExecutionContext;
use crate::errors::{CrushResult, error};
use crate::data::{ValueType, RowsReader, Row, Value};
use crate::stream::{Readable, ValueSender};
use crate::data::ColumnType;

pub fn run(mut input: impl Readable, sender: ValueSender) -> CrushResult<()> {
    let mut output_type = vec![ColumnType::named("idx", ValueType::Integer)];
    output_type.extend(input.types().clone());
    let output = sender.initialize(output_type)?;

    let mut line: i128 = 0;
    loop {
        match input.read() {
            Ok(row) => {
                let mut out = vec![Value::Integer(line)];
                out.extend(row.into_vec());
                output.send(Row::new(out))?;
                line += 1;
            }
            Err(_) => break,
        }
    }
    Ok(())
}

pub fn perform(context: ExecutionContext) -> CrushResult<()> {
    match context.input.recv()? {
        Value::Stream(s) => run(s.stream, context.output),
        Value::Rows(r) => run(RowsReader::new(r), context.output),
        _ => error("Expected a stream"),
    }
}