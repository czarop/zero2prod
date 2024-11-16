-- A table for idempotency keys - stored as :
    -- user_id
    -- idempotency key
    -- http response
    -- time of creation
-- http response will be split into various components:
    -- status code
    -- headers
    -- body


-- this is like a struct for sql - we can create these and store them in tables
-- useful when you can't store certain types of data
-- here we use it to store the different header name value pairs,
-- as postgres can't accept tuples
CREATE TYPE header_pair AS (
    name TEXT,
    value BYTEA
);
CREATE TABLE idempotency (
    user_id uuid NOT NULL REFERENCES users(user_id),
    idempotency_key TEXT NOT NULL,
    response_status_code SMALLINT NOT NULL,
    response_headers header_pair[] NOT NULL,
    response_body BYTEA NOT NULL,
    created_at timestamptz NOT NULL,
    PRIMARY KEY(user_id, idempotency_key)
);