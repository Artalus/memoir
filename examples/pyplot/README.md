Analyzing Memoir reports
---

One way of processing Memoir reports would be to load them into a Python script,
process the data (to extract something meaningful from commandline, for example),
and then plot the results.

This example uses [`plotly`](https://plotly.com/python/) do display interactive
charts from Python in your browser.

First prepare the virtual environment to run the example:
```bash
$ python -m venv venv
$ . venv/bin/activate
$ python -m pip install -r requirements.txt
```

Then run the script with some existing report as an input:
```bash
$ python plot.py --report ../docker/workdir/memoir.csv
```
