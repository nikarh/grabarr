# Security

grabarr has no authentication of its own and is meant to sit behind a reverse proxy that has it. Ways to get around
that are security issues: making grabarr act for a page of another site, reading its secrets, having it fetch or
delete what it shouldn't (anything beyond the torrents of its configured category), or breaking out of its
restrictions in the container.

Please report them privately, through
[GitHub's vulnerability reporting](https://github.com/nikarh/grabarr/security/advisories/new), not in a public
issue.
