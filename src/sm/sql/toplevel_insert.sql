insert into session_toplevel
    (session_id, user_id, name, name_text, data)
values (?1, ?2, ?3, ?4, ?5)
returning toplevel_id;
