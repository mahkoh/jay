pragma journal_mode = wal;
pragma synchronous = normal;
pragma busy_timeout = 1000;
pragma foreign_keys = on;
pragma recursive_triggers = off;
pragma temp_store = file;

begin;

create table if not exists user
(
    user_id integer primary key,
    name    blob not null
);

create index if not exists user_name on user (name);

create temp table user_to_delete
as
select name
from user;

create index user_to_delete_name on user_to_delete (name);

commit;

pragma optimize = 0x10002;
