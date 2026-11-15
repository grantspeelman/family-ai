-- `address` goes before `status`, because its CHECK reads `status`.
ALTER TABLE accounts DROP COLUMN fly_machine_id;
ALTER TABLE accounts DROP COLUMN fly_volume_id;
ALTER TABLE accounts DROP COLUMN fly_app_name;
ALTER TABLE accounts DROP COLUMN address;
ALTER TABLE accounts DROP COLUMN status;
