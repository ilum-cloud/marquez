# Database Migration Guide (PostgreSQL 14 to 16)

When upgrading Marquez to a version that uses PostgreSQL 16 (from 14), you must migrate your data because the underlying data files are incompatible between these major versions.

## Overview

We provide a helper script `docker/migrate-db.sh` to automate the backup and restore process.

The migration process involves:
1.  **Backup**: Running a temporary Postgres 14 container to dump your existing data.
2.  **Reset**: Removing the incompatible data volume.
3.  **Upgrade**: Starting the new Postgres 16 container.
4.  **Restore**: Importing the data into the new container.

## Prerequisites

*   Docker and Docker Compose installed.
*   The `docker/migrate-db.sh` script must be executable (`chmod +x docker/migrate-db.sh`).
*   **Stop** any running Marquez containers before starting.
    ```bash
    ./docker/down.sh
    ```

## Step-by-Step Guide

### 1. Backup Existing Data

Run the backup command. This will create a `marquez_dump.sql` file in your current directory.

```bash
./docker/migrate-db.sh backup
```

*   **Note**: This script will prompt you to delete the old volume after a successful backup. Say **Yes** (y) to proceed. If you say No, you must manually delete the volume before starting the new version.

### 2. Start Marquez (New Version)

Start the Marquez containers as usual. This will initialize a new, empty PostgreSQL 16 database.

```bash
./docker/up.sh
```

Wait for the containers to be fully up and running.

### 3. Restore Data

Once Marquez is running, restore your data from the dump file.

```bash
./docker/migrate-db.sh restore
```

This will import your existing data into the new database.

## Troubleshooting

*   **Volume Name**: If your database volume is named differently than `marquez_db-backup`, specify it with `-v`:
    ```bash
    ./docker/migrate-db.sh backup -v my_custom_volume_name
    ```
*   **Container Name**: If your database container is not named `marquez-db`, specify it with `-c`:
    ```bash
    ./docker/migrate-db.sh restore -c my_db_container