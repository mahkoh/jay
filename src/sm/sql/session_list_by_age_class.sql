select age_class, count(*)
from session
where user_id is null
group by age_class
order by age_class;
