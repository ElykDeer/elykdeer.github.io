---
layout: home
---
{::options parse_block_html="true" /}

<div style="height:5vh">
</div>
<div style="height:10vh">
# Hi, I'm Kyle Martin
{: style="text-align: center;"}
</div>

<div style="height:10vh">
## Here, on my website, you can find tons of info about
{: style="text-align: center;"}
</div>

<div style="height:75vh">
  <table style="align:center;width:100%;">
    <tr>
      <th>My Projects</th>
      <th> </th>
      <th>About Me</th>
    </tr>
    <tr>
      <th>
        <a href="/#projects" id="projectsImage"> <img src="/assets/projects.png" style="width:250px;height:250px;border:0;" />
        </a>
        <canvas id="circuitCanvas"> </canvas>
        {% include circuit.html %}
      </th>
      <th>
        or
      </th>
      <th>
        <a href="/#aboutme" id="aboutmeImage"> <img src="/assets/aboutme.png" style="width:250px;height:250px;border:0;" />
        </a>
        {% include heartbeat.html  %}
      </th>
    </tr>
  </table>
</div>

<div style="height:50vh">
# Projects {#projects}
{: style="text-align: center;"}
So here's some info about the projects I'm working on
</div>

<div style="height:50vh">
# About Me {#aboutme}
{: style="text-align: center;"}
So here's some info about me
</div>
