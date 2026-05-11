import { describe } from "bun:test";

describe("DBOBJ N-API Bindings - Full Operations", () => {
  // Each module registers its own tests via global test()
  import("./tests/crud.test");
  import("./tests/typed.test");
  import("./tests/sql.test");
  import("./tests/prepared.test");
  import("./tests/meta.test");
  import("./tests/transactions.test");
  import("./tests/errors.test");
});
