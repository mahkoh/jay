delete
from session_toplevel
where toplevel_id in (select tl.toplevel_id
                      from session_toplevel tl
                               join session s using (session_id)
                      where tl.name = ?1
                        and s.session = ?2
                        and s.user_id = ?3);
