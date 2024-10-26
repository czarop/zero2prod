# for windows, use CLI and just write the docker commands out

#!/usr/bin/env bash
set -x
set -eo pipefail

# check that sqlx is installed, and if not install it
if ! [ -x "$(command -v sqlx)" ]; then
    echo >&2 "Error: sqlx is not installed."
    echo >&2 "Use:"
    echo >&2 "cargo install --version='~0.8' sqlx-cli \
--no-default-features --features rustls,postgres"
    echo >&2 "to install it."
    exit 1
fi

# Check if a custom parameter has been set, otherwise use default values
DB_PORT="${POSTGRES_PORT:=5432}"
SUPERUSER="${SUPERUSER:=postgres}"
SUPERUSER_PWD="${SUPERUSER_PWD:=password}"

APP_USER="${APP_USER:=app}"
APP_USER_PWD="${APP_USER_PWD:=secret}"
APP_DB_NAME="${APP_DB_NAME:=newsletter}"

# Allow to skip Docker if a dockerized Postgres database is already running
if [[ -z "${SKIP_DOCKER}" ]]
then

    # Launch postgres using Docker
    CONTAINER_NAME="postgres"
    docker run \
        --env POSTGRES_USER=${SUPERUSER} \
        --env POSTGRES_PASSWORD=${SUPERUSER_PWD} \
        --health-cmd="pg_isready -U ${SUPERUSER} || exit 1" \
        --health-interval=1s \
        --health-timeout=5s \
        --health-retries=5 \
        --publish "${DB_PORT}":5432 \
        --detach \
        --name "${CONTAINER_NAME}" \
        postgres -N 1000
        # ^ Increased maximum number of connections for testing purposes

    # for windows smth like this, play with the quotation marks
    #docker run -e POSTGRES_USER=postgres -e POSTGRES_PASSWORD=password -p "5432":5432 -n "postgres" postgres -N 1000

    # Wait for Postgres to be ready to accept connections - not required
    until [ \
        "$(docker inspect -f "{{.State.Health.Status}}" ${CONTAINER_NAME})" == \
        "healthy" \
    ]; do
        >&2 echo "Postgres is still unavailable - sleeping"
        sleep 1
    done



    # for windows:
    #docker exec -it "postgres" psql -U "postgres" -c "CREATE USER app WITH PASSWORD 'secret';"
    #docker exec -it "postgres" psql -U "postgres" -c "ALTER USER app CREATEDB;"

    # Create the application user
    CREATE_QUERY="CREATE USER ${APP_USER} WITH PASSWORD '${APP_USER_PWD}';"
    docker exec -it "${CONTAINER_NAME}" psql -U "${SUPERUSER}" -c "${CREATE_QUERY}"
    # Grant create db privileges to the app user
    GRANT_QUERY="ALTER USER ${APP_USER} CREATEDB;"
    docker exec -it "${CONTAINER_NAME}" psql -U "${SUPERUSER}" -c "${GRANT_QUERY}"
fi

>&2 echo "Postgres is up and running on port ${DB_PORT} - running migrations now!"

# create the db in sqlx
DATABASE_URL=postgres://${APP_USER}:${APP_USER_PWD}@localhost:${DB_PORT}/${APP_DB_NAME}
export DATABASE_URL
# for windows
#set postgres://app:secret@localhost:5432/newsletter

# for windows do these in VScode (with migration table.sql file)
sqlx database create
sqlx migrate run

>&2 echo "Postgres has been migrated, ready to go!"

