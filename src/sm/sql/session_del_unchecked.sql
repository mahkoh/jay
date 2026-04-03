delete
from session
where session = ?1
returning session_id;
