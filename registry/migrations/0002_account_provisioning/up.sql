-- Provisioning state for each account's Fly app, volume and Machine.
--
-- Rows created before this migration have no Fly resources, so they default
-- to `failed`. The store inserts new rows as `provisioning`. `address` is the
-- family app's base URL and is set only when the status is `ready`.
--
-- The Fly ids are kept so later work can find or clean up the resources.
ALTER TABLE accounts ADD COLUMN status TEXT NOT NULL DEFAULT 'failed'
    CHECK (status IN ('provisioning', 'ready', 'failed'));
ALTER TABLE accounts ADD COLUMN address TEXT
    CHECK (address IS NULL OR status = 'ready');
ALTER TABLE accounts ADD COLUMN fly_app_name TEXT;
ALTER TABLE accounts ADD COLUMN fly_volume_id TEXT;
ALTER TABLE accounts ADD COLUMN fly_machine_id TEXT;
