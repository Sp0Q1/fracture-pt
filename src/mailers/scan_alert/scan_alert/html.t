<!DOCTYPE html>
<html>
<head><meta charset="utf-8"></head>
<body>
  <h2>Scan Changes Detected</h2>
  <p><strong>Scan:</strong> {{ scan_name }}</p>
  <p><strong>Domain:</strong> {{ domain }}</p>

  {% if added_subdomains %}
  <h3>New Subdomains</h3>
  <ul>
  {% for sub in added_subdomains %}
    <li>{{ sub }}</li>
  {% endfor %}
  </ul>
  {% endif %}

  {% if removed_subdomains %}
  <h3>Removed Subdomains</h3>
  <ul>
  {% for sub in removed_subdomains %}
    <li>{{ sub }}</li>
  {% endfor %}
  </ul>
  {% endif %}

  {% if newly_resolved %}
  <h3>Newly Resolved</h3>
  <ul>
  {% for sub in newly_resolved %}
    <li>{{ sub }}</li>
  {% endfor %}
  </ul>
  {% endif %}

  {% if newly_unresolved %}
  <h3>Newly Unresolved</h3>
  <ul>
  {% for sub in newly_unresolved %}
    <li>{{ sub }}</li>
  {% endfor %}
  </ul>
  {% endif %}

  {% if new_ports %}
  <h3>New Open Ports</h3>
  <ul>
  {% for port in new_ports %}
    <li>{{ port }}</li>
  {% endfor %}
  </ul>
  {% endif %}

  <hr>
  <p>GetHacked - Attack Surface Monitoring</p>
</body>
</html>