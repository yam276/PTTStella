// No saved secret is returned to these fields. Closing/revoking clears drafts.
function createLoginDialog({tr,send,stamp,prepare,restore,changed,focus}) {
    const loginButton=document.getElementById("login-open"),loginDialog=document.getElementById("login-dialog"),loginStop=document.getElementById("login-stop");
    const loginAccount=document.getElementById("login-account"),loginPassword=document.getElementById("login-password"),loginRemember=document.getElementById("login-remember");
    const loginSubmit=document.getElementById("login-submit"),loginForget=document.getElementById("login-forget"),loginCancel=document.getElementById("login-cancel"),loginNote=document.getElementById("login-note");
    let loginBusy=false,loginSaved=false,disposed=false;
    function loginWorking(busy){loginBusy=busy;loginSubmit.disabled=busy;loginForget.disabled=busy||!loginSaved;loginAccount.disabled=busy;loginPassword.disabled=busy;loginRemember.disabled=busy;}
    function openLogin(e){if(loginBusy||disposed)return;prepare(e);loginPassword.value="";loginRemember.checked=false;loginNote.textContent=tr("vault-reading");loginWorking(true);loginDialog.showModal();changed();send({kind:"login_open",trusted:e.isTrusted});}
    function submitLogin(e){if(loginBusy||!loginDialog.open)return;e.preventDefault();const password=loginPassword.value;loginPassword.value="";loginWorking(true);loginNote.textContent=tr("login-preparing");send({kind:"login_submit",account:loginAccount.value,password,remember:loginRemember.checked,stamp:stamp(),trusted:e.isTrusted});}
    function forgetLogin(e){if(loginBusy)return;loginPassword.value="";loginWorking(true);send({kind:"login_forget",trusted:e.isTrusted});}
    function closeLogin(e){e?.preventDefault();loginPassword.value="";loginRemember.checked=false;if(loginDialog.open)loginDialog.close();send({kind:"login_cancel",trusted:e?.isTrusted??false});restore();changed();}
    function stopLogin(e){send({kind:"login_cancel",trusted:e.isTrusted});focus();}

    function receive(result){
        if(disposed)return;
        if(result.state==="opened")loginSaved=result.saved;
        loginWorking(false);
        if(!loginDialog.open)return;
        switch(result.state){
            case "opened":loginAccount.value=result.account;loginNote.textContent=result.message;break;
            case "message":loginNote.textContent=result.message;break;
            case "sent":loginPassword.value="";loginDialog.close();restore();changed();return;
            case "idle":break;
            default:throw new Error("Unknown login result");
        }
        (loginAccount.value?loginPassword:loginAccount).focus();
    }
    function revoke(){disposed=true;loginPassword.value="";loginRemember.checked=false;}
    function destroy(){revoke();loginAccount.value="";loginSaved=false;if(loginDialog.open)loginDialog.close();}
    return {request:openLogin,receive,revoke,destroy,update:packet=>{loginStop.hidden=!packet.login_pending;loginButton.disabled=packet.ptt_features===false||loginButton.dataset.platformSupported==="false";},
        clean:()=>disposed&&!loginDialog.open&&!loginPassword.value&&!loginAccount.value&&!loginRemember.checked,
        bindings:[[loginButton,"click",openLogin],[loginSubmit,"click",submitLogin],[loginForget,"click",forgetLogin],
            [loginCancel,"click",closeLogin],[loginDialog,"cancel",closeLogin],[loginStop,"click",stopLogin]]};
}
