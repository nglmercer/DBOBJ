import { dlopen, FFIType, suffix, CString, ptr } from "bun:ffi";

const libPath =
  process.env.DBOBJ_LIB_PATH ||
  `${import.meta.dir}/../target/debug/libdbobj.${suffix}`;

const lib = dlopen(libPath, {
  dbobj_open: {
    args: [FFIType.cstring],
    returns: FFIType.pointer,
  },
  dbobj_close: {
    args: [FFIType.u64],
    returns: FFIType.pointer,
  },
  dbobj_execute: {
    args: [FFIType.u64, FFIType.cstring],
    returns: FFIType.pointer,
  },
  dbobj_create_table: {
    args: [FFIType.u64, FFIType.cstring, FFIType.cstring],
    returns: FFIType.pointer,
  },
  dbobj_insert: {
    args: [FFIType.u64, FFIType.cstring, FFIType.cstring],
    returns: FFIType.pointer,
  },
  dbobj_insert_batch: {
    args: [FFIType.u64, FFIType.cstring, FFIType.cstring],
    returns: FFIType.pointer,
  },
  dbobj_insert_object: {
    args: [FFIType.u64, FFIType.cstring, FFIType.cstring],
    returns: FFIType.pointer,
  },
  dbobj_select: {
    args: [FFIType.u64, FFIType.cstring, FFIType.cstring, FFIType.cstring],
    returns: FFIType.pointer,
  },
  dbobj_select_all: {
    args: [FFIType.u64, FFIType.cstring],
    returns: FFIType.pointer,
  },
  dbobj_update: {
    args: [FFIType.u64, FFIType.cstring, FFIType.cstring, FFIType.cstring],
    returns: FFIType.pointer,
  },
  dbobj_delete: {
    args: [FFIType.u64, FFIType.cstring, FFIType.cstring],
    returns: FFIType.pointer,
  },
  dbobj_create_index: {
    args: [FFIType.u64, FFIType.cstring, FFIType.cstring, FFIType.bool],
    returns: FFIType.pointer,
  },
  dbobj_list_tables: {
    args: [FFIType.u64],
    returns: FFIType.pointer,
  },
  dbobj_table_info: {
    args: [FFIType.u64, FFIType.cstring],
    returns: FFIType.pointer,
  },
  dbobj_save: {
    args: [FFIType.u64, FFIType.cstring],
    returns: FFIType.pointer,
  },
  dbobj_load: {
    args: [FFIType.cstring],
    returns: FFIType.pointer,
  },
  dbobj_free_string: {
    args: [FFIType.pointer],
    returns: FFIType.void,
  },
});

function callAndParse(fn: (...args: any[]) => number | null, ...args: any[]) {
  const ptr = fn(...args);
  if (ptr === null || ptr === 0) {
    throw new Error("Null pointer returned from FFI call");
  }
  const raw = new CString(ptr).toString();
  lib.symbols.dbobj_free_string(ptr);
  const parsed = JSON.parse(raw);
  if (parsed.error) {
    throw new Error(parsed.error);
  }
  return parsed.ok;
}

export type DatabaseHandle = number;

export function open(name: string): DatabaseHandle {
  return callAndParse(lib.symbols.dbobj_open, Buffer.from(name + "\0", "utf-8"));
}

export function close(handle: DatabaseHandle): void {
  callAndParse(lib.symbols.dbobj_close, handle);
}

export function execute(handle: DatabaseHandle, sql: string): any[] {
  return callAndParse(
    lib.symbols.dbobj_execute,
    handle,
    Buffer.from(sql + "\0", "utf-8"),
  );
}

export function createTable(
  handle: DatabaseHandle,
  name: string,
  columns: { name: string; type: string; nullable?: boolean }[],
): void {
  callAndParse(
    lib.symbols.dbobj_create_table,
    handle,
    Buffer.from(name + "\0", "utf-8"),
    Buffer.from(JSON.stringify(columns) + "\0", "utf-8"),
  );
}

export function insert(
  handle: DatabaseHandle,
  table: string,
  values: any[],
): string {
  return callAndParse(
    lib.symbols.dbobj_insert,
    handle,
    Buffer.from(table + "\0", "utf-8"),
    Buffer.from(JSON.stringify(values) + "\0", "utf-8"),
  );
}

export function insertBatch(
  handle: DatabaseHandle,
  table: string,
  rows: any[][],
): string[] {
  return callAndParse(
    lib.symbols.dbobj_insert_batch,
    handle,
    Buffer.from(table + "\0", "utf-8"),
    Buffer.from(JSON.stringify(rows) + "\0", "utf-8"),
  );
}

export function insertObject(
  handle: DatabaseHandle,
  table: string,
  data: Record<string, any>,
): string {
  return callAndParse(
    lib.symbols.dbobj_insert_object,
    handle,
    Buffer.from(table + "\0", "utf-8"),
    Buffer.from(JSON.stringify(data) + "\0", "utf-8"),
  );
}

export function select(
  handle: DatabaseHandle,
  table: string,
  column: string,
  value: any,
): Record<string, any>[] {
  return callAndParse(
    lib.symbols.dbobj_select,
    handle,
    Buffer.from(table + "\0", "utf-8"),
    Buffer.from(column + "\0", "utf-8"),
    Buffer.from(JSON.stringify(value) + "\0", "utf-8"),
  );
}

export function selectAll(
  handle: DatabaseHandle,
  table: string,
): Record<string, any>[] {
  return callAndParse(
    lib.symbols.dbobj_select_all,
    handle,
    Buffer.from(table + "\0", "utf-8"),
  );
}

export function update(
  handle: DatabaseHandle,
  table: string,
  id: string,
  values: any[],
): void {
  callAndParse(
    lib.symbols.dbobj_update,
    handle,
    Buffer.from(table + "\0", "utf-8"),
    Buffer.from(id + "\0", "utf-8"),
    Buffer.from(JSON.stringify(values) + "\0", "utf-8"),
  );
}

export function deleteRow(
  handle: DatabaseHandle,
  table: string,
  id: string,
): void {
  callAndParse(
    lib.symbols.dbobj_delete,
    handle,
    Buffer.from(table + "\0", "utf-8"),
    Buffer.from(id + "\0", "utf-8"),
  );
}

export function createIndex(
  handle: DatabaseHandle,
  table: string,
  column: string,
  unique: boolean = false,
): void {
  callAndParse(
    lib.symbols.dbobj_create_index,
    handle,
    Buffer.from(table + "\0", "utf-8"),
    Buffer.from(column + "\0", "utf-8"),
    unique,
  );
}

export function listTables(handle: DatabaseHandle): string[] {
  return callAndParse(lib.symbols.dbobj_list_tables, handle);
}

export function tableInfo(
  handle: DatabaseHandle,
  table: string,
): { name: string; columns: any[]; row_count: number } {
  return callAndParse(
    lib.symbols.dbobj_table_info,
    handle,
    Buffer.from(table + "\0", "utf-8"),
  );
}

export function save(handle: DatabaseHandle, path: string): void {
  callAndParse(
    lib.symbols.dbobj_save,
    handle,
    Buffer.from(path + "\0", "utf-8"),
  );
}

export function load(path: string): DatabaseHandle {
  return callAndParse(
    lib.symbols.dbobj_load,
    Buffer.from(path + "\0", "utf-8"),
  );
}

export { lib };
