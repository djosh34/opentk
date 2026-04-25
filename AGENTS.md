Never ignore the linter, the linters are there with good reason.
Skipping tests is one of the worst things you can do, giving extremely false confidence. Never skip a test, if something is missing in order to test -> fail.

Never swallow/ignore any errors. That is a huge anti-pattern, and must be reported as add-bug task.

This is greenfield project with 0 users. 
We don't have legacy at all. If you find any legacy code/docs, remove it.
No backwards compatibility allowed!
You are encouraged to make large refactors and schema changes

