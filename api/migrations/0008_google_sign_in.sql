-- Users who sign in with Google have no password.

alter table users alter column password_hash drop not null;
