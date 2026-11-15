//! The tables as `migrations/` leaves them, for Diesel's typed queries. Update
//! this with every migration that changes a table.

diesel::table! {
    notes (id) {
        id -> BigInt,
        body -> Text,
        created_at -> Text,
    }
}
