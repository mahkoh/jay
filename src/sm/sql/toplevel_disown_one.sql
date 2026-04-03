update session_toplevel
set user_id = null
where toplevel_id = ?1 and user_id = ?2;
