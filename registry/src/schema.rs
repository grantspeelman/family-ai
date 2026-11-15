//! The tables as `migrations/` leaves them, for Diesel's typed queries. Update
//! this with every migration that changes a table.

diesel::table! {
    accounts (id) {
        id -> BigInt,
        email -> Text,
        email_key -> Text,
        family_name -> Text,
        status -> Text,
        address -> Nullable<Text>,
        fly_app_name -> Nullable<Text>,
        fly_volume_id -> Nullable<Text>,
        fly_machine_id -> Nullable<Text>,
    }
}
