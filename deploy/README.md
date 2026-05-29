# Local database

1. Copy env template:

    ```bash
    cp deploy/.env.example deploy/.env
    ```

2. Start PostgreSQL:

    ```bash
    docker compose -f deploy/docker-compose.yml --env-file deploy/.env up -d
    ```

3. Set server env and run migrations:

    ```bash
    export DATABASE_URL=postgres://rsmsg:rsmsg_dev_password@127.0.0.1:5432/rsmsg
    cargo install sqlx-cli --no-default-features --features rustls,postgres
    sqlx migrate run --source crates/server/migrations
    ```

4. Run server:

    ```bash
    cargo run -p server
    ```

    To bind all interfaces:

    ```bash
    cargo run -p server -- -ip 0.0.0.0:3000
    ```
