---
name: Something is wrong
about: 'Something is not working as expected.'
title: ''
labels: question
assignees: ''

---

This template is for situations where you expect something to work but it doesn't. You do not have to use it for feature requests.

Attach the full log file from the session during which the problem occurred: `jay log --path`.

If the problem occurred during one session but not during another, attach the log file from the working session as well.

If the problem relates to a particular client:

- Start the client with `WAYLAND_DEBUG=1`, redirect stderr to a file, and attach that file.
- If the problem is only present sometimes: attach two files: one created while the problem was present and one while it was absent.

If the problem relates to display devices:

- Attach a file containing the full `jay randr` output.
- Attach a file containing the full `drm_info` output.
- If the problem is only present sometimes: attach two files each: one created while the problem was present and one while it was absent.

If the problem relates to input devices:

- Attach a file containing the full `jay input show -v` output.
- If the problem is only present sometimes: attach two files: one created while the problem was present and one while it was absent.

If you know that some of this does not relate to your problem, disregard it.
