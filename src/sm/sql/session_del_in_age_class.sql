delete
from session
where session_id in (select session_id
                     from session
                     where user_id is null
                       and age_class = ?1
                     order by atime
                     limit ?2);
