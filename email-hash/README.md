# email-hash

For rent redistribution within a community, it's important to know which user accounts are real. Since rents are valuable, there's the risk that people will try to cheat the system by creating duplicate accounts and unfairly capture more than their equal share of the community's rents.

Community membership is defined by a list of known member emails. Once a member has created an account and verified that they own the email (by clicking a verification link sent to their inbox), their membership in the community is checked against the community email list. If their email or a hash of their email is present, then they're eligible to receive a share of the community's rents.

Hashing the emails provides a small measure of increased privacy, since membership is not revealed to the admins of the tinylvt instance until that person voluntarily creates an account. The admins could test emails they think might be present, but this requires some pre-existing knowledge of possible entries.
