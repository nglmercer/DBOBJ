use ahash::HashMapExt;
use dbobj::{Id, Value};
use dbobj_server::protocol::{
    ColumnDef, ComparisonOp, ExprData, Request, Response, SerializedRow,
};

/// Helper to create a RowData (HashMap) with column-value pairs
fn make_row(pairs: Vec<(&str, Value)>) -> dbobj::RowData {
    pairs.into_iter().map(|(k, v)| (k.into(), v)).collect()
}

#[test]
fn test_request_serialization_roundtrip() {
    let requests = vec![
        Request::CreateTable {
            name: "users".into(),
            columns: vec![
                ColumnDef {
                    name: "id".into(),
                    data_type: "Integer".into(),
                    nullable: false,
                },
                ColumnDef {
                    name: "name".into(),
                    data_type: "String".into(),
                    nullable: true,
                },
            ],
        },
        Request::DropTable {
            name: "users".into(),
        },
        Request::ListTables,
        Request::TableInfo {
            name: "users".into(),
        },
        Request::Insert {
            table: "users".into(),
            data: make_row(vec![]),
            custom_id: None,
        },
        Request::InsertValues {
            table: "users".into(),
            values: vec![Value::Integer(42)],
        },
        Request::InsertBatch {
            table: "users".into(),
            batch: vec![],
        },
        Request::InsertBatchValues {
            table: "users".into(),
            batch: vec![],
        },
        Request::InsertOrReplace {
            table: "users".into(),
            values: vec![],
            unique_column: "id".into(),
        },
        Request::UpdateRow {
            table: "users".into(),
            id: Id::Integer(1),
            data: make_row(vec![]),
        },
        Request::UpdateValues {
            table: "users".into(),
            id: Id::Integer(1),
            values: vec![Value::Integer(99)],
        },
        Request::UpdateByIndices {
            table: "users".into(),
            id: Id::Integer(1),
            updates: vec![(0, Value::Integer(99))],
        },
        Request::DeleteRow {
            table: "users".into(),
            id: Id::Integer(1),
        },
        Request::DeleteBatch {
            table: "users".into(),
            ids: vec![Id::Integer(1), Id::Integer(2)],
        },
        Request::Query {
            table: "users".into(),
            column_name: "age".into(),
            value: Value::Integer(30),
        },
        Request::QueryPredicate {
            table: "users".into(),
            column_idx: 1,
            operator: ComparisonOp::Gt,
            value: Value::Integer(18),
        },
        Request::QueryExpr {
            table: "users".into(),
            expr: ExprData::Binary {
                left: Box::new(ExprData::Column("age".into())),
                op: ComparisonOp::Gt,
                right: Box::new(ExprData::Literal(Value::Integer(18))),
            },
        },
        Request::CreateIndex {
            table: "users".into(),
            column: "name".into(),
        },
        Request::CreateUniqueIndex {
            table: "users".into(),
            column: "email".into(),
        },
        Request::HashJoin {
            table1: "users".into(),
            col1: "id".into(),
            table2: "orders".into(),
            col2: "user_id".into(),
        },
        Request::BeginTransaction,
        Request::CommitTransaction,
        Request::RollbackTransaction,
        Request::Save,
        Request::Load {
            path: "test.db".into(),
        },
        Request::Ping,
    ];

    for req in &requests {
        let encoded = bincode::serialize(req).expect("Failed to serialize request");
        let decoded: Request = bincode::deserialize(&encoded).expect("Failed to deserialize request");
        assert_eq!(
            format!("{:?}", req),
            format!("{:?}", decoded),
            "Roundtrip failed for request: {:?}",
            req
        );
    }
}

#[test]
fn test_response_serialization_roundtrip() {
    let responses = vec![
        Response::Ok(42),
        Response::Rows(vec![SerializedRow {
            id: Id::Integer(1),
            data: vec![Value::Integer(10), Value::String("hello".into())],
        }]),
        Response::TableList(vec!["users".into(), "orders".into()]),
        Response::TableInfo {
            name: "users".into(),
            columns: vec![ColumnDef {
                name: "id".into(),
                data_type: "Integer".into(),
                nullable: false,
            }],
            row_count: 100,
        },
        Response::Id(Id::Integer(1)),
        Response::Ids(vec![Id::Integer(1), Id::Integer(2)]),
        Response::JoinedRows(vec![(
            SerializedRow {
                id: Id::Integer(1),
                data: vec![Value::Integer(1)],
            },
            SerializedRow {
                id: Id::Integer(10),
                data: vec![Value::Integer(1), Value::Integer(100)],
            },
        )]),
        Response::Pong,
        Response::Error("something went wrong".into()),
    ];

    for resp in &responses {
        let encoded = bincode::serialize(resp).expect("Failed to serialize response");
        let decoded: Response =
            bincode::deserialize(&encoded).expect("Failed to deserialize response");
        assert_eq!(
            format!("{:?}", resp),
            format!("{:?}", decoded),
            "Roundtrip failed for response"
        );
    }
}

#[test]
fn test_expr_data_roundtrip() {
    let exprs = vec![
        ExprData::Column("age".into()),
        ExprData::Literal(Value::Integer(25)),
        ExprData::Binary {
            left: Box::new(ExprData::Column("age".into())),
            op: ComparisonOp::Gt,
            right: Box::new(ExprData::Literal(Value::Integer(18))),
        },
        ExprData::And(vec![
            ExprData::Binary {
                left: Box::new(ExprData::Column("age".into())),
                op: ComparisonOp::Gt,
                right: Box::new(ExprData::Literal(Value::Integer(18))),
            },
            ExprData::Binary {
                left: Box::new(ExprData::Column("status".into())),
                op: ComparisonOp::Eq,
                right: Box::new(ExprData::Literal(Value::String("active".into()))),
            },
        ]),
        ExprData::Or(vec![
            ExprData::Binary {
                left: Box::new(ExprData::Column("role".into())),
                op: ComparisonOp::Eq,
                right: Box::new(ExprData::Literal(Value::String("admin".into()))),
            },
            ExprData::Binary {
                left: Box::new(ExprData::Column("role".into())),
                op: ComparisonOp::Eq,
                right: Box::new(ExprData::Literal(Value::String("moderator".into()))),
            },
        ]),
        ExprData::Not(Box::new(ExprData::Column("deleted".into()))),
    ];

    for expr in &exprs {
        let encoded = bincode::serialize(expr).expect("Failed to serialize ExprData");
        let decoded: ExprData =
            bincode::deserialize(&encoded).expect("Failed to deserialize ExprData");
        assert_eq!(
            format!("{:?}", expr),
            format!("{:?}", decoded),
            "Roundtrip failed for ExprData"
        );
    }
}

#[test]
fn test_comparison_op_values() {
    let ops = vec![
        (ComparisonOp::Eq, "Eq"),
        (ComparisonOp::Neq, "Neq"),
        (ComparisonOp::Gt, "Gt"),
        (ComparisonOp::Gte, "Gte"),
        (ComparisonOp::Lt, "Lt"),
        (ComparisonOp::Lte, "Lte"),
    ];

    for (op, name) in &ops {
        let encoded = bincode::serialize(op).expect("Failed to serialize ComparisonOp");
        let decoded: ComparisonOp =
            bincode::deserialize(&encoded).expect("Failed to deserialize ComparisonOp");
        assert_eq!(
            format!("{:?}", decoded),
            *name,
            "ComparisonOp variant mismatch"
        );
    }
}

#[test]
fn test_large_batch_roundtrip() {
    let mut batch = Vec::with_capacity(1000);
    for i in 0..1000 {
        let mut row = dbobj::RowData::new();
        row.insert("value".into(), Value::Integer(i as i64));
        batch.push(row);
    }

    let req = Request::InsertBatch {
        table: "large_table".into(),
        batch,
    };

    let encoded = bincode::serialize(&req).expect("Failed to serialize large batch");
    let _decoded: Request =
        bincode::deserialize(&encoded).expect("Failed to deserialize large batch");

    assert!(encoded.len() > 1000, "Large batch should be > 1KB");
    assert!(encoded.len() < 50_000, "Large batch should be < 50KB");
}