begin;

-- session

create table if not exists session
(
    session_id integer primary key,
    session    blob    not null unique,
    user_id    integer references user (user_id) on delete set null,
    data       blob    not null,
    ctime      integer not null default (unixepoch()),
    atime      integer not null default (unixepoch()),
    age_class  integer not null default (0)
);

create index if not exists session_user_id
    on session (user_id)
    where user_id is not null;

create index if not exists session_age_class
    on session (age_class, atime)
    where user_id is null;

create temp trigger session_update_atime
    after update of user_id, data
    on main.session
    for each row
begin
    update session
    set age_class = cast(log2(max(16, unixepoch() - ctime)) as int) - 4,
        atime     = unixepoch()
    where session_id = new.session_id;
end;

-- session_toplevel

create table if not exists session_toplevel
(
    toplevel_id integer primary key,
    session_id  integer not null references session (session_id) on delete cascade,
    user_id     integer references user (user_id) on delete set null,
    name        blob    not null unique,
    name_text   blob    not null,
    data        blob    not null,
    ctime       integer not null default (unixepoch()),
    atime       integer not null default (unixepoch()),
    age_class   integer not null default (0)
);

create index if not exists session_toplevel_session_id
    on session_toplevel (session_id);

create index if not exists session_toplevel_session_id_user_id
    on session_toplevel (session_id, user_id)
    where user_id is not null;

create index if not exists session_toplevel_user_id
    on session_toplevel (user_id)
    where user_id is not null;

create index if not exists session_toplevel_age_class
    on session_toplevel (age_class, atime)
    where user_id is null;

create temp trigger session_toplevel_update_atime
    after update of user_id, name, data
    on main.session_toplevel
    for each row
begin
    update session_toplevel
    set age_class = cast(log2(max(16, unixepoch() - ctime)) as int) - 4,
        atime     = unixepoch()
    where toplevel_id = new.toplevel_id;
    update session
    set age_class = cast(log2(max(16, unixepoch() - ctime)) as int) - 4,
        atime     = unixepoch()
    where session_id = new.session_id;
end;

commit;

pragma optimize = 0x10002;
