select exists(select * from session where session_id = ?1 and user_id = ?2);
