insert into session
    (session, user_id, data)
values (?1, ?2, ?3)
on conflict do update set user_id = ?2,
                          data    = ?3
returning session_id;
