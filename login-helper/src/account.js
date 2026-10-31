const auth = await fetch('/api/', {method:'POST', credentials:'include'});
if (auth.status === 401) return JSON.stringify({authenticated:false});
if (!auth.ok) return JSON.stringify({error:'account_state_failed'});
const state = await auth.json();
if (state.authenticated !== true) return JSON.stringify({authenticated:false});
const headers = {'Content-Type':'application/json'};
const xsrf = document.cookie.match(/(?:^|;\s*)XSRF-TOKEN=([^;]+)/);
if (xsrf) headers['X-XSRF-TOKEN'] = decodeURIComponent(xsrf[1]);
const response = await fetch('/api/details', {credentials:'include', headers});
if (!response.ok) return JSON.stringify({error:'account_profile_failed'});
const profile = await response.json();
return JSON.stringify({authenticated:true,
    account_id: state.accountId == null ? null : String(state.accountId),
    battle_tag: typeof profile.battleTag === 'string' ? profile.battleTag : null});
