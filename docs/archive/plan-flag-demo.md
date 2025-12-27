# --plan Flag Demonstration

This document provides a practical demonstration of the `--plan` flag in Rustible.

## Example Playbook

```yaml
---
- name: Web Server Setup
  hosts: localhost
  gather_facts: false
  vars:
    web_package: nginx
    web_port: 80
  tasks:
    - name: Install web server
      package:
        name: "{{ web_package }}"
        state: present
      tags:
        - install

    - name: Configure web server
      template:
        src: nginx.conf.j2
        dest: /etc/nginx/nginx.conf
      notify: Reload nginx
      tags:
        - configure

    - name: Start web server
      service:
        name: "{{ web_package }}"
        state: started
        enabled: true
      tags:
        - service

- name: Application Deployment
  hosts: localhost
  gather_facts: false
  tasks:
    - name: Clone application repository
      git:
        repo: https://github.com/example/app.git
        dest: /opt/app

    - name: Create application user
      user:
        name: appuser
        state: present
        shell: /bin/bash

    - name: Run deployment script
      command: /opt/app/deploy.sh
      when: deployment_required

  handlers:
    - name: Reload nginx
      service:
        name: nginx
        state: reloaded
```

## Running with --plan Flag

### Command
```bash
rustible run playbook.yml --plan -e "deployment_required=true" -v
```

### Output
```
===========================
  PLAYBOOK: playbook.yml
===========================

INFO: Loading playbook...
WARNING: No inventory specified, using localhost
WARNING: Running in PLAN MODE - showing execution plan only

EXECUTION PLAN
--------------
INFO: Rustible will perform the following actions:

INFO: [Play 1/2] ⚡ Web Server Setup
INFO:   Hosts: localhost (1 host)
INFO:   Tasks: 3 tasks
INFO:
  ▸ Task 1/3: Install web server
INFO:     Module: package
INFO:       [localhost] will install package: {{ web_package }}
INFO:
  ▸ Task 2/3: Configure web server
INFO:     Module: template
INFO:       [localhost] will render template nginx.conf.j2 to /etc/nginx/nginx.conf
INFO:     Notify: Reload nginx
INFO:
  ▸ Task 3/3: Start web server
INFO:     Module: service
INFO:       [localhost] will started service: {{ web_package }}
INFO:
[Play 2/2] ⚡ Application Deployment
INFO:   Hosts: localhost (1 host)
INFO:   Tasks: 3 tasks
INFO:
  ▸ Task 1/3: Clone application repository
INFO:     Module: git
INFO:       [localhost] will clone/update https://github.com/example/app.git to /opt/app
INFO:
  ▸ Task 2/3: Create application user
INFO:     Module: user
INFO:       [localhost] will create/update user: appuser
INFO:
  ▸ Task 3/3: Run deployment script
INFO:     Module: command
INFO:       [localhost] will execute: /opt/app/deploy.sh
INFO:     When: deployment_required


PLAN SUMMARY
-------------
INFO: Plan: 6 tasks across 1 host
INFO:
To execute this plan, run the same command without --plan

PLAY RECAP **********************************************************************

Playbook run took 0ms
INFO: Playbook finished in 0.00s
```

## Key Observations

### 1. No Execution
Notice that the plan completes in 0ms - no actual work is being done, no SSH connections are made.

### 2. Detailed Information
For each task, you can see:
- Task name and number
- Module being used
- What action will be performed
- Which hosts will be affected

### 3. Handler Notifications
Task 2 shows: `Notify: Reload nginx`
This tells you that when this task makes changes, it will trigger the "Reload nginx" handler.

### 4. Conditional Display
Task 6 shows: `When: deployment_required`
This indicates the task will only run if the condition is met.

### 5. Play Summary
```
[Play 1/2] ⚡ Web Server Setup
  Hosts: localhost (1 host)
  Tasks: 3 tasks
```
Clear overview of what will happen in each play.

### 6. Plan Summary
```
Plan: 6 tasks across 1 host
To execute this plan, run the same command without --plan
```
Final summary with next steps.

## Common Use Cases

### 1. Pre-Production Validation
```bash
# Check what will happen before running against production
rustible run production.yml --plan -i inventory/production.yml
```

### 2. Testing Tag Filters
```bash
# See which tasks will run with specific tags
rustible run playbook.yml --plan --tags install,configure
```

### 3. Variable Testing
```bash
# Verify variable substitution before execution
rustible run playbook.yml --plan -e "env=staging" -e "version=2.0"
```

### 4. Quick Playbook Review
```bash
# Get an overview of what a playbook does
rustible run unknown-playbook.yml --plan
```

### 5. Documentation Generation
```bash
# Generate execution plan as documentation
rustible run playbook.yml --plan > execution-plan.txt
```

## Comparison: With vs Without --plan

### Without --plan (Normal Execution)
```bash
rustible run playbook.yml
```
- Connects to remote hosts via SSH
- Executes tasks and makes changes
- Takes time proportional to task complexity
- Requires proper credentials and network access
- Can have side effects

### With --plan
```bash
rustible run playbook.yml --plan
```
- No SSH connections
- No actual changes
- Completes almost instantly
- No credentials needed
- Zero side effects
- Safe for production review

## Best Practices

### 1. Always Plan First
```bash
# Bad: Running directly without review
rustible run critical-playbook.yml -i production.yml

# Good: Review plan first, then execute
rustible run critical-playbook.yml -i production.yml --plan
rustible run critical-playbook.yml -i production.yml
```

### 2. Use with Verbose Mode
```bash
# Get more details in the plan
rustible run playbook.yml --plan -vv
```

### 3. Combine with Tag Filtering
```bash
# Plan only specific tasks
rustible run playbook.yml --plan --tags database
```

### 4. Verify Variable Substitution
```bash
# Check if variables are set correctly
rustible run playbook.yml --plan -e @vars/production.yml
```

### 5. Save Plans for Review
```bash
# Save plan for team review
rustible run playbook.yml --plan > plan-$(date +%Y%m%d).txt
```

## Advanced Examples

### Multi-Environment Planning
```bash
# Compare what will happen in different environments
rustible run playbook.yml --plan -e @vars/dev.yml > plan-dev.txt
rustible run playbook.yml --plan -e @vars/staging.yml > plan-staging.txt
rustible run playbook.yml --plan -e @vars/prod.yml > plan-prod.txt
diff plan-dev.txt plan-staging.txt
```

### CI/CD Integration
```bash
# In your CI/CD pipeline
if rustible run playbook.yml --plan -i inventory/production.yml; then
  echo "Plan looks good, proceeding with execution..."
  rustible run playbook.yml -i inventory/production.yml
else
  echo "Plan failed, aborting deployment"
  exit 1
fi
```

### Change Approval Workflow
```bash
# Generate plan for approval
rustible run playbook.yml --plan -i production.yml > change-request.txt

# After approval, execute
rustible run playbook.yml -i production.yml
```

## Tips and Tricks

### 1. Understanding Output Symbols
- `⚡` - Play marker
- `▸` - Task marker
- `[hostname]` - Target host
- `When:` - Conditional execution
- `Notify:` - Handler trigger

### 2. Reading Task Counts
```
Tasks: 5 tasks
```
This is the total number of tasks in the play, not necessarily how many will run (tags/conditions may skip some).

### 3. Host Count Accuracy
```
Plan: 10 tasks across 5 hosts
```
This shows the total tasks that would run across all matched hosts.

### 4. No Output for Skipped Tasks
If you use `--tags` or `--skip-tags`, tasks that won't run don't appear in the plan.

### 5. Handlers in Plan
Handlers are mentioned when tasks notify them, but they don't appear as separate items in the plan.

## Troubleshooting

### Issue: Plan shows no tasks
**Cause**: Tag filters or limit patterns excluded all tasks
**Solution**: Check your `--tags` or `--limit` arguments

### Issue: Variables not resolved
**Cause**: Variables from facts or dynamic sources
**Solution**: Variables from Ansible facts won't be available in plan mode since no connection is made

### Issue: Plan output too verbose
**Cause**: High verbosity level
**Solution**: Reduce verbosity or remove `-v` flags

### Issue: Want to save plan to file
**Solution**: Redirect output: `rustible run playbook.yml --plan > plan.txt`

## Conclusion

The `--plan` flag is a powerful feature that allows you to:
- Preview playbook execution safely
- Validate playbooks before running
- Understand complex playbooks quickly
- Document what a playbook will do
- Test variable substitution
- Review handler notifications
- Verify conditional logic

Use it regularly to avoid surprises and ensure your playbooks do exactly what you expect!
