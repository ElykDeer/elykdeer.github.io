---
layout: home
---

[](Just some space)
{: style="min-height:5vh;"}

# Hi, I'm Kyle Martin
{: style="text-align:center;min-height:10vh;"}

## Here, on my website, you can find tons of info about
{: style="text-align:center;min-height:10vh;"}

{::options parse_block_html="true" /}
<div style="min-height:75vh;">
  <table style="align:center;width:100%;">
    <tr>
      <th>Me</th>
      <th> </th>
      <th>My Projects</th>
    </tr>
    <tr>
      <th>
        <a href="/#aboutme" id="aboutmeImage"> <img src="/assets/aboutme.png" style="width:250px;height:250px;border:0;" />
        </a>
        {% include heartbeat.html  %}
      </th>
      <th>
        or
      </th>
      <th>
        <a href="/#projects" id="projectsImage"> <img src="/assets/projects.png" style="width:250px;height:250px;border:0;" />
        </a>
        <canvas id="circuitCanvas"> </canvas>
        {% include circuit.html %}
      </th>
    </tr>
  </table>
</div>

{::options parse_block_html="false" /}

# About Me {#aboutme}
{: style="text-align: center;"}
{% include aboutme.md %}

[](Just some space)
{: style="min-height:10vh;"}

# Projects {#projects}
{: style="text-align: center;"}
{% include projects.md %}
