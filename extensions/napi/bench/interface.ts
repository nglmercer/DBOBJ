export interface TestSuite {
  name: string;
  insert(count: number): number;
  readColumn(tableName: string, colName: string): number;
  find(tableName: string, colName: string, value: any): number;
  update(tableName: string, count: number): number;
  join(t1: string, c1: string, t2: string, c2: string): number;
}
