select age_class, count(*)
from session_toplevel
where user_id is null
group by age_class
order by age_class;
