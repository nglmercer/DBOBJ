#!/bin/bash
# setup_postgres.sh - Ephemeral Postgres for Benchmarking

PG_DATA="./pg_bench_data"
PG_PORT=5433
PG_USER=$(whoami)

# Stop any existing server
if [ -d "$PG_DATA" ]; then
    pg_ctl -D "$PG_DATA" stop -m fast || true
    rm -rf "$PG_DATA"
fi

echo "Initializing temporary Postgres data directory..."
initdb -D "$PG_DATA" --auth=trust --nosync

echo "Starting Postgres on port $PG_PORT..."
pg_ctl -D "$PG_DATA" -o "-p $PG_PORT" -l "$PG_DATA/logfile" start

# Wait for start
sleep 2

echo "Creating benchmark database..."
createdb -p $PG_PORT bench_db || true

echo "------------------------------------------------"
echo "Postgres is ready at: postgresql://$PG_USER@localhost:$PG_PORT/bench_db"
echo "To stop: pg_ctl -D $PG_DATA stop"
echo "------------------------------------------------"
