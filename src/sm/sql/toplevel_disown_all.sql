update session_toplevel
set user_id = null
where session_id = ?1 and user_id = ?2;
