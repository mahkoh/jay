delete
from user
where name in (select name from user_to_delete);
