const { Database } = require('./index');

const db = new Database("NodeDB");

// 1. Create table
db.createTable("users", ["id", "age"], ["integer", "integer"]);

// 2. Insert some data
console.log("Inserting rows...");
for (let i = 0; i < 100000; i++) {
    db.insertRowI64("users", [BigInt(i), BigInt(i * 2)]);
}

// 3. Get column zero-copy
console.log("Fetching 'age' column zero-copy...");
const start = Date.now();
const ages = db.getColumnI64("users", "age");
const end = Date.now();

console.log(`Column size: ${ages.length}`);
console.log(`First value: ${ages[0]}`);
console.log(`Last value: ${ages[ages.length - 1]}`);
console.log(`Time taken to wrap memory: ${end - start}ms`);

// Demonstrate direct access
let sum = 0n;
for (let i = 0; i < ages.length; i++) {
    sum += ages[i];
}
console.log(`Sum of ages: ${sum}`);
