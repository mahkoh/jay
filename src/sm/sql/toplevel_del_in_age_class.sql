delete
from session_toplevel
where toplevel_id in (select toplevel_id
                      from session_toplevel
                      where user_id is null
                        and age_class = ?1
                      order by atime
                      limit ?2);
