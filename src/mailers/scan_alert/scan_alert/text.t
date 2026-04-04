Scan Changes Detected

Scan: {{ scan_name }}
Domain: {{ domain }}

{% if added_subdomains %}New Subdomains:
{% for sub in added_subdomains %}- {{ sub }}
{% endfor %}{% endif %}
{% if removed_subdomains %}Removed Subdomains:
{% for sub in removed_subdomains %}- {{ sub }}
{% endfor %}{% endif %}
{% if newly_resolved %}Newly Resolved:
{% for sub in newly_resolved %}- {{ sub }}
{% endfor %}{% endif %}
{% if newly_unresolved %}Newly Unresolved:
{% for sub in newly_unresolved %}- {{ sub }}
{% endfor %}{% endif %}
{% if new_ports %}New Open Ports:
{% for port in new_ports %}- {{ port }}
{% endfor %}{% endif %}
--
GetHacked - Attack Surface Monitoring